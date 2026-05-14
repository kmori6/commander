use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::domain::error::session_repository_error::SessionRepositoryError;
use crate::domain::model::session::Session;
use crate::domain::repository::session_repository::SessionRepository;

#[derive(Clone)]
pub struct PostgresSessionRepository {
    pool: PgPool,
}

impl PostgresSessionRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[derive(sqlx::FromRow)]
struct SessionRow {
    id: Uuid,
    title: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl TryFrom<SessionRow> for Session {
    type Error = SessionRepositoryError;

    fn try_from(row: SessionRow) -> Result<Self, Self::Error> {
        Ok(Self {
            id: row.id,
            title: row.title,
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
    }
}

fn map_sqlx_error(err: sqlx::Error) -> SessionRepositoryError {
    SessionRepositoryError::Unexpected(err.to_string())
}

#[async_trait]
impl SessionRepository for PostgresSessionRepository {
    async fn create(&self, title: Option<String>) -> Result<Session, SessionRepositoryError> {
        let row = sqlx::query_as::<_, SessionRow>(
            r#"
            INSERT INTO sessions (title)
            VALUES ($1)
            RETURNING id, title, created_at, updated_at
            "#,
        )
        .bind(title.and_then(Session::normalize_title))
        .fetch_one(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        row.try_into()
    }

    async fn find_by_id(&self, id: Uuid) -> Result<Option<Session>, SessionRepositoryError> {
        let row = sqlx::query_as::<_, SessionRow>(
            r#"
            SELECT id, title, created_at, updated_at
            FROM sessions
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        row.map(TryInto::try_into).transpose()
    }

    async fn list_recent(&self, limit: usize) -> Result<Vec<Session>, SessionRepositoryError> {
        let limit = i64::try_from(limit)
            .map_err(|_| SessionRepositoryError::Unexpected(format!("invalid limit: {limit}")))?;

        let rows = sqlx::query_as::<_, SessionRow>(
            r#"
            SELECT id, title, created_at, updated_at
            FROM sessions
            ORDER BY updated_at DESC, id DESC
            LIMIT $1
            "#,
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        rows.into_iter().map(TryInto::try_into).collect()
    }

    async fn update_title(
        &self,
        id: Uuid,
        title: Option<String>,
    ) -> Result<Session, SessionRepositoryError> {
        let row = sqlx::query_as::<_, SessionRow>(
            r#"
            UPDATE sessions
            SET title = $2,
                updated_at = NOW()
            WHERE id = $1
            RETURNING id, title, created_at, updated_at
            "#,
        )
        .bind(id)
        .bind(title.and_then(Session::normalize_title))
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        row.ok_or(SessionRepositoryError::NotFound(id))?.try_into()
    }
}
