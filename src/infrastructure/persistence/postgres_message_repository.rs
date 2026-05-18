use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde_json::Value;
use sqlx::PgPool;
use std::collections::HashMap;
use uuid::Uuid;

use crate::domain::error::message_repository_error::MessageRepositoryError;
use crate::domain::model::message::{Message, MessageContent, Role, ToolCallOutputStatus};
use crate::domain::repository::message_repository::MessageRepository;

#[derive(Clone)]
pub struct PostgresMessageRepository {
    pool: PgPool,
}

impl PostgresMessageRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[derive(sqlx::FromRow)]
struct MessageRow {
    id: Uuid,
    task_id: Uuid,
    role: String,
    created_at: DateTime<Utc>,
}

#[derive(sqlx::FromRow)]
struct MessageContentRow {
    message_id: Uuid,
    content_type: String,
    text: Option<String>,
    call_id: Option<String>,
    tool_name: Option<String>,
    arguments: Option<Value>,
    output: Option<Value>,
    output_status: Option<String>,
}

fn map_sqlx_error(err: sqlx::Error) -> MessageRepositoryError {
    MessageRepositoryError::Unexpected(err.to_string())
}

fn content_row_to_model(row: MessageContentRow) -> Result<MessageContent, MessageRepositoryError> {
    match row.content_type.as_str() {
        "input_text" => Ok(MessageContent::InputText {
            text: row.text.unwrap_or_default(),
        }),
        "output_text" => Ok(MessageContent::OutputText {
            text: row.text.unwrap_or_default(),
        }),
        "tool_call" => Ok(MessageContent::ToolCall {
            call_id: row.call_id.unwrap_or_default(),
            tool_name: row.tool_name.unwrap_or_default(),
            arguments: row.arguments.unwrap_or(Value::Null),
        }),
        "tool_call_output" => {
            let status = row
                .output_status
                .as_deref()
                .and_then(ToolCallOutputStatus::from_db)
                .unwrap_or(ToolCallOutputStatus::Success);

            Ok(MessageContent::ToolCallOutput {
                call_id: row.call_id.unwrap_or_default(),
                output: row.output.unwrap_or(Value::Null),
                status,
            })
        }
        other => Err(MessageRepositoryError::Unexpected(format!(
            "unknown message content type: {other}"
        ))),
    }
}

fn build_message(
    row: MessageRow,
    content_rows: Vec<MessageContentRow>,
) -> Result<Message, MessageRepositoryError> {
    let role = Role::from_db(&row.role)
        .ok_or_else(|| MessageRepositoryError::Unexpected(format!("unknown role: {}", row.role)))?;

    let contents = content_rows
        .into_iter()
        .map(content_row_to_model)
        .collect::<Result<Vec<_>, _>>()?;

    Ok(Message::new(
        row.id,
        row.task_id,
        role,
        contents,
        row.created_at,
    ))
}

fn content_to_db(
    content: &MessageContent,
) -> Result<
    (
        &'static str,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<Value>,
        Option<Value>,
        Option<&'static str>,
    ),
    MessageRepositoryError,
> {
    match content {
        MessageContent::InputText { text } => Ok((
            "input_text",
            Some(text.clone()),
            None,
            None,
            None,
            None,
            None,
        )),
        MessageContent::InputImage { .. } | MessageContent::InputFile { .. } => {
            Err(MessageRepositoryError::InvalidMessage(
                "input_image and input_file are runtime-only contents and cannot be persisted"
                    .to_string(),
            ))
        }
        MessageContent::OutputText { text } => Ok((
            "output_text",
            Some(text.clone()),
            None,
            None,
            None,
            None,
            None,
        )),
        MessageContent::ToolCall {
            call_id,
            tool_name,
            arguments,
        } => Ok((
            "tool_call",
            None,
            Some(call_id.clone()),
            Some(tool_name.clone()),
            Some(arguments.clone()),
            None,
            None,
        )),
        MessageContent::ToolCallOutput {
            call_id,
            output,
            status,
        } => Ok((
            "tool_call_output",
            None,
            Some(call_id.clone()),
            None,
            None,
            Some(output.clone()),
            Some(status.as_str()),
        )),
    }
}

#[async_trait]
impl MessageRepository for PostgresMessageRepository {
    async fn save(
        &self,
        task_id: Uuid,
        role: Role,
        contents: Vec<MessageContent>,
    ) -> Result<Message, MessageRepositoryError> {
        if contents.iter().any(|content| !content.is_persistable()) {
            return Err(MessageRepositoryError::InvalidMessage(
                "input_image and input_file are runtime-only contents and cannot be persisted"
                    .to_string(),
            ));
        }

        let mut tx = self.pool.begin().await.map_err(map_sqlx_error)?;

        let session_id = sqlx::query_scalar::<_, Option<Uuid>>(
            r#"
            UPDATE tasks
            SET updated_at = NOW()
            WHERE id = $1
            RETURNING session_id
            "#,
        )
        .bind(task_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(map_sqlx_error)?;

        let Some(session_id) = session_id else {
            return Err(MessageRepositoryError::TaskNotFound(task_id));
        };

        if let Some(session_id) = session_id {
            sqlx::query(
                r#"
                UPDATE sessions
                SET updated_at = NOW()
                WHERE id = $1
                "#,
            )
            .bind(session_id)
            .execute(&mut *tx)
            .await
            .map_err(map_sqlx_error)?;
        }

        let row = sqlx::query_as::<_, MessageRow>(
            r#"
            INSERT INTO messages (task_id, role)
            VALUES ($1, $2)
            RETURNING id, task_id, role, created_at
            "#,
        )
        .bind(task_id)
        .bind(role.as_str())
        .fetch_one(&mut *tx)
        .await
        .map_err(map_sqlx_error)?;

        for (index, content) in contents.iter().enumerate() {
            let (content_type, text, call_id, tool_name, arguments, output, output_status) =
                content_to_db(content)?;

            sqlx::query(
                r#"
                INSERT INTO message_contents (
                  message_id, content_index, type, text, call_id, tool_name,
                  arguments, output, output_status
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
                "#,
            )
            .bind(row.id)
            .bind(index as i32)
            .bind(content_type)
            .bind(text)
            .bind(call_id)
            .bind(tool_name)
            .bind(arguments)
            .bind(output)
            .bind(output_status)
            .execute(&mut *tx)
            .await
            .map_err(map_sqlx_error)?;
        }

        tx.commit().await.map_err(map_sqlx_error)?;

        Ok(Message::new(
            row.id,
            row.task_id,
            role,
            contents,
            row.created_at,
        ))
    }

    async fn find_by_id(&self, id: Uuid) -> Result<Option<Message>, MessageRepositoryError> {
        let row = sqlx::query_as::<_, MessageRow>(
            r#"
            SELECT id, task_id, role, created_at
            FROM messages
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        let Some(row) = row else {
            return Ok(None);
        };

        let content_rows = sqlx::query_as::<_, MessageContentRow>(
            r#"
            SELECT
              message_id,
              type AS content_type,
              text,
              call_id,
              tool_name,
              arguments,
              output,
              output_status
            FROM message_contents
            WHERE message_id = $1
            ORDER BY content_index ASC
            "#,
        )
        .bind(id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        build_message(row, content_rows).map(Some)
    }

    async fn list_for_task(&self, task_id: Uuid) -> Result<Vec<Message>, MessageRepositoryError> {
        let message_rows = sqlx::query_as::<_, MessageRow>(
            r#"
            SELECT id, task_id, role, created_at
            FROM messages
            WHERE task_id = $1
            ORDER BY created_at ASC, id ASC
            "#,
        )
        .bind(task_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        if message_rows.is_empty() {
            return Ok(Vec::new());
        }

        let message_ids = message_rows.iter().map(|row| row.id).collect::<Vec<_>>();

        let content_rows = sqlx::query_as::<_, MessageContentRow>(
            r#"
            SELECT
              message_id,
              type AS content_type,
              text,
              call_id,
              tool_name,
              arguments,
              output,
              output_status
            FROM message_contents
            WHERE message_id = ANY($1)
            ORDER BY message_id ASC, content_index ASC
            "#,
        )
        .bind(&message_ids)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        let mut contents_by_message = HashMap::<Uuid, Vec<MessageContentRow>>::new();
        for content_row in content_rows {
            contents_by_message
                .entry(content_row.message_id)
                .or_default()
                .push(content_row);
        }

        message_rows
            .into_iter()
            .map(|row| {
                let content_rows = contents_by_message.remove(&row.id).unwrap_or_default();
                build_message(row, content_rows)
            })
            .collect()
    }

    async fn find_tool_call_content_id(
        &self,
        message_id: Uuid,
        call_id: &str,
    ) -> Result<Option<Uuid>, MessageRepositoryError> {
        sqlx::query_scalar::<_, Uuid>(
            r#"
            SELECT id
            FROM message_contents
            WHERE message_id = $1
            AND type = 'tool_call'
            AND call_id = $2
            ORDER BY content_index ASC
            LIMIT 1
            "#,
        )
        .bind(message_id)
        .bind(call_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx_error)
    }

    async fn has_tool_output(
        &self,
        task_id: Uuid,
        call_id: &str,
    ) -> Result<bool, MessageRepositoryError> {
        let exists = sqlx::query_scalar::<_, bool>(
            r#"
            SELECT EXISTS (
            SELECT 1
            FROM messages m
            JOIN message_contents c ON c.message_id = m.id
            WHERE m.task_id = $1
                AND m.role = 'user'
                AND c.type = 'tool_call_output'
                AND c.call_id = $2
            )
            "#,
        )
        .bind(task_id)
        .bind(call_id)
        .fetch_one(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        Ok(exists)
    }
}
