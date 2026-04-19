use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::domain::error::chat_repository_error::ChatRepositoryError;
use crate::domain::model::chat_session::ChatSession;
use crate::domain::repository::chat_session_repository::ChatSessionRepository;

#[derive(Clone)]
pub struct PostgresChatSessionRepository {
    pool: PgPool,
}

impl PostgresChatSessionRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[derive(sqlx::FromRow)]
struct ChatSessionRow {
    id: Uuid,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl From<ChatSessionRow> for ChatSession {
    fn from(row: ChatSessionRow) -> Self {
        Self {
            id: row.id,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

fn map_sqlx_error(err: sqlx::Error) -> ChatRepositoryError {
    ChatRepositoryError::Unexpected(err.to_string())
}

#[async_trait]
impl ChatSessionRepository for PostgresChatSessionRepository {
    async fn create(&self) -> Result<ChatSession, ChatRepositoryError> {
        let row = sqlx::query_as::<_, ChatSessionRow>(
            r#"
            INSERT INTO chat_sessions DEFAULT VALUES
            RETURNING id, created_at, updated_at
            "#,
        )
        .fetch_one(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        Ok(row.into())
    }

    async fn find_by_id(&self, id: Uuid) -> Result<Option<ChatSession>, ChatRepositoryError> {
        let row = sqlx::query_as::<_, ChatSessionRow>(
            r#"
            SELECT id, created_at, updated_at
            FROM chat_sessions
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        Ok(row.map(Into::into))
    }

    async fn list_recent(&self, limit: usize) -> Result<Vec<ChatSession>, ChatRepositoryError> {
        let rows = sqlx::query_as::<_, ChatSessionRow>(
            r#"
            SELECT id, created_at, updated_at
            FROM chat_sessions
            ORDER BY updated_at DESC
            LIMIT $1
            "#,
        )
        .bind(limit as i64)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        Ok(rows.into_iter().map(Into::into).collect())
    }
}
