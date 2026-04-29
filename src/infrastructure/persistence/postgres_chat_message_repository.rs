use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde_json::{Value, json};
use sqlx::PgPool;
use uuid::Uuid;

use crate::domain::error::chat_repository_error::ChatRepositoryError;
use crate::domain::model::chat_message::ChatMessage;
use crate::domain::model::message::{Message, MessageContent};
use crate::domain::model::role::Role;
use crate::domain::model::tool::{ToolCall, ToolResultMessage};
use crate::domain::repository::chat_message_repository::{
    ChatMessageRepository, ChatMessageSummary,
};

#[derive(sqlx::FromRow)]
struct ChatMessageSummaryRow {
    session_id: Uuid,
    first_user_message: Option<String>,
    message_count: i64,
}

impl From<ChatMessageSummaryRow> for ChatMessageSummary {
    fn from(row: ChatMessageSummaryRow) -> Self {
        Self {
            session_id: row.session_id,
            first_user_message: row.first_user_message,
            message_count: row.message_count,
        }
    }
}

#[derive(Clone)]
pub struct PostgresChatMessageRepository {
    pool: PgPool,
}

impl PostgresChatMessageRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[derive(sqlx::FromRow)]
struct ChatMessageRow {
    id: Uuid,
    session_id: Uuid,
    role: String,
    kind: String,
    text: Option<String>,
    payload: Option<Value>,
    created_at: DateTime<Utc>,
}

impl TryFrom<ChatMessageRow> for ChatMessage {
    type Error = ChatRepositoryError;

    fn try_from(row: ChatMessageRow) -> Result<Self, Self::Error> {
        Ok(Self {
            id: row.id,
            session_id: row.session_id,
            message: restore_message(&row.role, &row.kind, row.text, row.payload)?,
            created_at: row.created_at,
        })
    }
}

fn map_sqlx_error(err: sqlx::Error) -> ChatRepositoryError {
    ChatRepositoryError::Unexpected(err.to_string())
}

fn role_to_db(role: Role) -> &'static str {
    match role {
        Role::System => "system",
        Role::User => "user",
        Role::Assistant => "assistant",
        Role::Tool => "tool",
    }
}

fn role_from_db(value: &str) -> Result<Role, ChatRepositoryError> {
    match value {
        "system" => Ok(Role::System),
        "user" => Ok(Role::User),
        "assistant" => Ok(Role::Assistant),
        "tool" => Ok(Role::Tool),
        _ => Err(ChatRepositoryError::Unexpected(format!(
            "unknown role: {value}"
        ))),
    }
}

fn split_message(message: &Message) -> (String, Option<String>, Option<Value>) {
    match &message.content {
        MessageContent::Text(text) => ("text".to_string(), Some(text.clone()), None),
        MessageContent::Multimodal { text, .. } => ("text".to_string(), Some(text.clone()), None),
        MessageContent::ToolCall { text, tool_calls } => (
            "tool_call".to_string(),
            text.clone(),
            Some(json!({
                "tool_calls": tool_calls.iter().map(|call| {
                    json!({
                        "id": call.id,
                        "name": call.name,
                        "arguments": call.arguments,
                    })
                }).collect::<Vec<_>>()
            })),
        ),
        MessageContent::ToolResults(results) => (
            "tool_results".to_string(),
            None,
            Some(json!({
                "tool_results": results.iter().map(|result| {
                    json!({
                        "tool_call_id": result.tool_call_id,
                        "output": result.output,
                        "is_error": result.is_error,
                    })
                }).collect::<Vec<_>>()
            })),
        ),
    }
}

fn restore_message(
    role: &str,
    kind: &str,
    text: Option<String>,
    payload: Option<Value>,
) -> Result<Message, ChatRepositoryError> {
    let role = role_from_db(role)?;

    match kind {
        "text" => Ok(Message {
            role,
            content: MessageContent::Text(text.unwrap_or_default()),
        }),
        "tool_call" => {
            let tool_calls = payload
                .as_ref()
                .and_then(|v| v.get("tool_calls"))
                .and_then(Value::as_array)
                .ok_or_else(|| {
                    ChatRepositoryError::Unexpected("missing tool_calls payload".into())
                })?
                .iter()
                .map(|item| {
                    Ok(ToolCall {
                        id: item
                            .get("id")
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_string(),
                        name: item
                            .get("name")
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_string(),
                        arguments: item.get("arguments").cloned().unwrap_or(Value::Null),
                    })
                })
                .collect::<Result<Vec<_>, ChatRepositoryError>>()?;

            Ok(Message {
                role,
                content: MessageContent::ToolCall { text, tool_calls },
            })
        }
        "tool_results" => {
            let results = payload
                .as_ref()
                .and_then(|v| v.get("tool_results"))
                .and_then(Value::as_array)
                .ok_or_else(|| {
                    ChatRepositoryError::Unexpected("missing tool_results payload".into())
                })?
                .iter()
                .map(|item| {
                    Ok(ToolResultMessage {
                        tool_call_id: item
                            .get("tool_call_id")
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_string(),
                        output: item.get("output").cloned().unwrap_or(Value::Null),
                        is_error: item
                            .get("is_error")
                            .and_then(Value::as_bool)
                            .unwrap_or(false),
                    })
                })
                .collect::<Result<Vec<_>, ChatRepositoryError>>()?;

            Ok(Message {
                role,
                content: MessageContent::ToolResults(results),
            })
        }
        _ => Err(ChatRepositoryError::Unexpected(format!(
            "unknown kind: {kind}"
        ))),
    }
}

#[async_trait]
impl ChatMessageRepository for PostgresChatMessageRepository {
    async fn append(
        &self,
        session_id: Uuid,
        message: Message,
    ) -> Result<ChatMessage, ChatRepositoryError> {
        let (kind, text, payload) = split_message(&message);
        let role = role_to_db(message.role).to_string();

        // transaction: 1. update chat_sessions.updated_at -> 2. insert into chat_messages
        let mut tx = self.pool.begin().await.map_err(map_sqlx_error)?;

        let updated = sqlx::query_scalar::<_, Uuid>(
            r#"
            UPDATE chat_sessions
            SET updated_at = NOW()
            WHERE id = $1
            RETURNING id
            "#,
        )
        .bind(session_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(map_sqlx_error)?;

        if updated.is_none() {
            return Err(ChatRepositoryError::SessionNotFound(session_id));
        }

        let row = sqlx::query_as::<_, ChatMessageRow>(
            r#"
            INSERT INTO chat_messages (session_id, role, kind, text, payload)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING id, session_id, role, kind, text, payload, created_at
            "#,
        )
        .bind(session_id)
        .bind(role)
        .bind(kind)
        .bind(text)
        .bind(payload)
        .fetch_one(&mut *tx)
        .await
        .map_err(map_sqlx_error)?;

        tx.commit().await.map_err(map_sqlx_error)?;

        row.try_into()
    }

    async fn list_for_session(
        &self,
        session_id: Uuid,
    ) -> Result<Vec<ChatMessage>, ChatRepositoryError> {
        let rows = sqlx::query_as::<_, ChatMessageRow>(
            r#"
            SELECT id, session_id, role, kind, text, payload, created_at
            FROM chat_messages
            WHERE session_id = $1
            ORDER BY created_at ASC, id ASC
            "#,
        )
        .bind(session_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        rows.into_iter().map(TryInto::try_into).collect()
    }

    async fn summarize_by_session_ids(
        &self,
        session_ids: &[Uuid],
    ) -> Result<Vec<ChatMessageSummary>, ChatRepositoryError> {
        if session_ids.is_empty() {
            return Ok(Vec::new());
        }

        let rows = sqlx::query_as::<_, ChatMessageSummaryRow>(
            r#"
            SELECT
                counts.session_id,
                first_user_message.text AS first_user_message,
                counts.message_count
            FROM (
                SELECT
                    session_id,
                    COUNT(*) AS message_count
                FROM chat_messages
                WHERE session_id = ANY($1::uuid[])
                GROUP BY session_id
            ) counts
            LEFT JOIN LATERAL (
                SELECT text
                FROM chat_messages
                WHERE session_id = counts.session_id
                AND role = 'user'
                AND text IS NOT NULL
                ORDER BY created_at ASC, id ASC
                LIMIT 1
            ) first_user_message ON true
            "#,
        )
        .bind(session_ids.to_vec())
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        Ok(rows.into_iter().map(Into::into).collect())
    }
}
