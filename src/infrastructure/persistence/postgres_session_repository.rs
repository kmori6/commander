use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::domain::error::session_repository_error::SessionRepositoryError;
use crate::domain::model::session::{Session, SessionKind, SessionStatus};
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
    kind: String,
    title: Option<String>,
    status: String,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl TryFrom<SessionRow> for Session {
    type Error = SessionRepositoryError;

    fn try_from(row: SessionRow) -> Result<Self, Self::Error> {
        let kind = SessionKind::from_db(&row.kind).ok_or_else(|| {
            SessionRepositoryError::Unexpected(format!("unknown session kind: {}", row.kind))
        })?;

        let status = SessionStatus::from_db(&row.status).ok_or_else(|| {
            SessionRepositoryError::Unexpected(format!("unknown session status: {}", row.status))
        })?;

        Ok(Self {
            id: row.id,
            kind,
            title: row.title,
            status,
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
    async fn create(
        &self,
        kind: SessionKind,
        title: Option<String>,
    ) -> Result<Session, SessionRepositoryError> {
        let row = sqlx::query_as::<_, SessionRow>(
            r#"
            INSERT INTO sessions (kind, title)
            VALUES ($1, $2)
            RETURNING id, kind, title, status, created_at, updated_at
            "#,
        )
        .bind(kind.as_str())
        .bind(title.and_then(Session::normalize_title))
        .fetch_one(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        row.try_into()
    }

    async fn find_by_id(&self, id: Uuid) -> Result<Option<Session>, SessionRepositoryError> {
        let row = sqlx::query_as::<_, SessionRow>(
            r#"
            SELECT id, kind, title, status, created_at, updated_at
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

    async fn list_recent(
        &self,
        kind: Option<SessionKind>,
        limit: usize,
    ) -> Result<Vec<Session>, SessionRepositoryError> {
        let limit = i64::try_from(limit)
            .map_err(|_| SessionRepositoryError::Unexpected(format!("invalid limit: {limit}")))?;

        let rows = sqlx::query_as::<_, SessionRow>(
            r#"
            SELECT id, kind, title, status, created_at, updated_at
            FROM sessions
            WHERE ($1::TEXT IS NULL OR kind = $1)
            ORDER BY updated_at DESC, id DESC
            LIMIT $2
            "#,
        )
        .bind(kind.map(SessionKind::as_str))
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
            RETURNING id, kind, title, status, created_at, updated_at
            "#,
        )
        .bind(id)
        .bind(title.and_then(Session::normalize_title))
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        row.ok_or(SessionRepositoryError::NotFound(id))?.try_into()
    }

    async fn update_status(
        &self,
        id: Uuid,
        status: SessionStatus,
    ) -> Result<Session, SessionRepositoryError> {
        let row = sqlx::query_as::<_, SessionRow>(
            r#"
            UPDATE sessions
            SET status = $2,
                updated_at = NOW()
            WHERE id = $1
            RETURNING id, kind, title, status, created_at, updated_at
            "#,
        )
        .bind(id)
        .bind(status.as_str())
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        row.ok_or(SessionRepositoryError::NotFound(id))?.try_into()
    }
}
