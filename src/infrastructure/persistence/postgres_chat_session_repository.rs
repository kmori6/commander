use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::domain::error::chat_repository_error::ChatRepositoryError;
use crate::domain::model::chat_session::{ChatSession, ChatSessionStatus};
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
    title: Option<String>,
    status: String,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl TryFrom<ChatSessionRow> for ChatSession {
    type Error = ChatRepositoryError;

    fn try_from(row: ChatSessionRow) -> Result<Self, Self::Error> {
        let status = ChatSessionStatus::from_db(&row.status).ok_or_else(|| {
            ChatRepositoryError::Unexpected(format!("unknown chat session status: {}", row.status))
        })?;

        Ok(Self {
            id: row.id,
            title: row.title,
            status,
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
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
            RETURNING id, title, status, created_at, updated_at
            "#,
        )
        .fetch_one(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        Ok(row.try_into()?)
    }

    async fn find_by_id(&self, id: Uuid) -> Result<Option<ChatSession>, ChatRepositoryError> {
        let row = sqlx::query_as::<_, ChatSessionRow>(
            r#"
            SELECT id, title, status, created_at, updated_at
            FROM chat_sessions
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        Ok(row.map(TryInto::try_into).transpose()?)
    }

    async fn list_recent(&self, limit: usize) -> Result<Vec<ChatSession>, ChatRepositoryError> {
        let rows = sqlx::query_as::<_, ChatSessionRow>(
            r#"
            SELECT id, title, status, created_at, updated_at
            FROM chat_sessions
            ORDER BY updated_at DESC
            LIMIT $1
            "#,
        )
        .bind(limit as i64)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        rows.into_iter().map(TryInto::try_into).collect()
    }

    async fn delete_by_id(&self, id: Uuid) -> Result<(), ChatRepositoryError> {
        let result = sqlx::query(
            r#"
            DELETE FROM chat_sessions
            WHERE id = $1
            "#,
        )
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        if result.rows_affected() == 0 {
            return Err(ChatRepositoryError::SessionNotFound(id));
        }

        Ok(())
    }

    async fn update_status(
        &self,
        id: Uuid,
        status: ChatSessionStatus,
    ) -> Result<ChatSession, ChatRepositoryError> {
        let row = sqlx::query_as::<_, ChatSessionRow>(
            r#"
            UPDATE chat_sessions
            SET status = $2, updated_at = NOW()
            WHERE id = $1
            RETURNING id, title, status, created_at, updated_at
            "#,
        )
        .bind(id)
        .bind(status.as_str())
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        let Some(row) = row else {
            return Err(ChatRepositoryError::SessionNotFound(id));
        };

        row.try_into()
    }

    async fn update_title(
        &self,
        id: Uuid,
        title: String,
    ) -> Result<ChatSession, ChatRepositoryError> {
        let row = sqlx::query_as::<_, ChatSessionRow>(
            r#"
            UPDATE chat_sessions
            SET title = $2, updated_at = NOW()
            WHERE id = $1
            RETURNING id, title, status, created_at, updated_at
            "#,
        )
        .bind(id)
        .bind(title)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        let Some(row) = row else {
            return Err(ChatRepositoryError::SessionNotFound(id));
        };

        row.try_into()
    }
}
