use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::{PgPool, types::Json};
use uuid::Uuid;

use crate::domain::error::message_repository_error::MessageRepositoryError;
use crate::domain::model::message::{Message, MessageContent, MessageUsage, Role, TaskUsage};
use crate::domain::repository::message_repository::MessageRepository;

#[derive(Clone)]
pub struct PostgresMessageRepository {
    pool: PgPool,
}

impl PostgresMessageRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    async fn save_message(
        &self,
        task_id: Uuid,
        role: Role,
        contents: Vec<MessageContent>,
        model: Option<String>,
        usage: Option<MessageUsage>,
    ) -> Result<Message, MessageRepositoryError> {
        validate_contents(&contents)?;

        if let Some(model) = model.as_deref()
            && model.trim().is_empty()
        {
            return Err(MessageRepositoryError::InvalidMessage(
                "model must not be empty".to_string(),
            ));
        }

        if let Some(usage) = usage {
            validate_usage(usage)?;
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
            INSERT INTO messages (task_id, role, contents, model, usage)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING id, task_id, role, contents, model, usage, created_at
            "#,
        )
        .bind(task_id)
        .bind(role.as_str())
        .bind(Json(contents))
        .bind(model)
        .bind(usage.map(Json))
        .fetch_one(&mut *tx)
        .await
        .map_err(map_sqlx_error)?;

        tx.commit().await.map_err(map_sqlx_error)?;

        row_to_message(row)
    }
}

#[derive(sqlx::FromRow)]
struct MessageRow {
    id: Uuid,
    task_id: Uuid,
    role: String,
    contents: Json<Vec<MessageContent>>,
    model: Option<String>,
    usage: Option<Json<MessageUsage>>,
    created_at: DateTime<Utc>,
}

#[derive(sqlx::FromRow)]
struct TaskUsageRow {
    input_tokens: i64,
    output_tokens: i64,
    cache_read_tokens: i64,
    cache_write_tokens: i64,
}

fn map_sqlx_error(err: sqlx::Error) -> MessageRepositoryError {
    MessageRepositoryError::Unexpected(err.to_string())
}

fn row_to_message(row: MessageRow) -> Result<Message, MessageRepositoryError> {
    let role = Role::from_db(&row.role)
        .ok_or_else(|| MessageRepositoryError::Unexpected(format!("unknown role: {}", row.role)))?;

    Ok(Message::new(
        row.id,
        row.task_id,
        role,
        row.contents.0,
        row.model,
        row.usage.map(|usage| usage.0),
        row.created_at,
    ))
}

fn validate_contents(contents: &[MessageContent]) -> Result<(), MessageRepositoryError> {
    if contents.iter().any(|content| !content.is_persistable()) {
        return Err(MessageRepositoryError::InvalidMessage(
            "input_image and input_file are runtime-only contents and cannot be persisted"
                .to_string(),
        ));
    }

    Ok(())
}

fn validate_usage(usage: MessageUsage) -> Result<(), MessageRepositoryError> {
    if usage.input_tokens < 0
        || usage.output_tokens < 0
        || usage.cache_read_tokens < 0
        || usage.cache_write_tokens < 0
    {
        return Err(MessageRepositoryError::InvalidMessage(
            "usage token counts must be greater than or equal to zero".to_string(),
        ));
    }

    Ok(())
}

#[async_trait]
impl MessageRepository for PostgresMessageRepository {
    async fn save(
        &self,
        task_id: Uuid,
        role: Role,
        contents: Vec<MessageContent>,
    ) -> Result<Message, MessageRepositoryError> {
        self.save_message(task_id, role, contents, None, None).await
    }

    async fn save_response(
        &self,
        task_id: Uuid,
        contents: Vec<MessageContent>,
        model: &str,
        usage: MessageUsage,
    ) -> Result<Message, MessageRepositoryError> {
        self.save_message(
            task_id,
            Role::Assistant,
            contents,
            Some(model.trim().to_string()),
            Some(usage),
        )
        .await
    }

    async fn find_by_id(&self, id: Uuid) -> Result<Option<Message>, MessageRepositoryError> {
        let row = sqlx::query_as::<_, MessageRow>(
            r#"
            SELECT id, task_id, role, contents, model, usage, created_at
            FROM messages
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        row.map(row_to_message).transpose()
    }

    async fn list_for_task(&self, task_id: Uuid) -> Result<Vec<Message>, MessageRepositoryError> {
        let rows = sqlx::query_as::<_, MessageRow>(
            r#"
            SELECT id, task_id, role, contents, model, usage, created_at
            FROM messages
            WHERE task_id = $1
            ORDER BY created_at ASC, id ASC
            "#,
        )
        .bind(task_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        rows.into_iter().map(row_to_message).collect()
    }

    async fn task_usage(&self, task_id: Uuid) -> Result<TaskUsage, MessageRepositoryError> {
        let row = sqlx::query_as::<_, TaskUsageRow>(
            r#"
            SELECT
              COALESCE(SUM((usage->>'input_tokens')::BIGINT), 0)::BIGINT AS input_tokens,
              COALESCE(SUM((usage->>'output_tokens')::BIGINT), 0)::BIGINT AS output_tokens,
              COALESCE(SUM((usage->>'cache_read_tokens')::BIGINT), 0)::BIGINT AS cache_read_tokens,
              COALESCE(SUM((usage->>'cache_write_tokens')::BIGINT), 0)::BIGINT AS cache_write_tokens
            FROM messages
            WHERE task_id = $1
              AND usage IS NOT NULL
            "#,
        )
        .bind(task_id)
        .fetch_one(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        Ok(TaskUsage {
            task_id,
            input_tokens: row.input_tokens,
            output_tokens: row.output_tokens,
            cache_read_tokens: row.cache_read_tokens,
            cache_write_tokens: row.cache_write_tokens,
        })
    }
}
