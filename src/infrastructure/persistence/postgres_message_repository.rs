use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::{PgPool, types::Json};
use uuid::Uuid;

use crate::domain::error::message_repository_error::MessageRepositoryError;
use crate::domain::model::message::{
    Message, MessageContent, MessageUsage, NewMessage, Role, TaskUsage,
};
use crate::domain::repository::message_repository::MessageRepository;

#[derive(Clone)]
pub struct PostgresMessageRepository {
    pool: PgPool,
}

impl PostgresMessageRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    async fn save_message(&self, message: NewMessage) -> Result<Message, MessageRepositoryError> {
        let NewMessage {
            task_id,
            role,
            contents,
            model,
            usage,
        } = message;

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

    Message::restore(
        row.id,
        row.task_id,
        role,
        row.contents.0,
        row.model,
        row.usage.map(|usage| usage.0),
        row.created_at,
    )
    .map_err(Into::into)
}

#[async_trait]
impl MessageRepository for PostgresMessageRepository {
    async fn save(&self, message: NewMessage) -> Result<Message, MessageRepositoryError> {
        self.save_message(message).await
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

    async fn list_for_session(
        &self,
        session_id: Uuid,
        until_task_id: Option<Uuid>,
    ) -> Result<Vec<Message>, MessageRepositoryError> {
        let rows = sqlx::query_as::<_, MessageRow>(
            r#"
            SELECT m.id, m.task_id, m.role, m.contents, m.model, m.usage, m.created_at
            FROM messages m
            INNER JOIN tasks t
              ON t.id = m.task_id
            LEFT JOIN tasks until_task
              ON until_task.id = $2
              AND until_task.session_id = $1
            WHERE t.session_id = $1
              AND (
                $2::UUID IS NULL
                OR (
                  until_task.id IS NOT NULL
                  AND (
                    t.created_at < until_task.created_at
                    OR (t.created_at = until_task.created_at AND t.id <= until_task.id)
                  )
                )
              )
            ORDER BY t.created_at ASC, t.id ASC, m.created_at ASC, m.id ASC
            "#,
        )
        .bind(session_id)
        .bind(until_task_id)
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
