use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::domain::error::tool_approval_repository_error::ToolApprovalRepositoryError;
use crate::domain::model::tool::{ToolApproval, ToolApprovalStatus};
use crate::domain::repository::tool_approval_repository::ToolApprovalRepository;

#[derive(Clone)]
pub struct PostgresToolApprovalRepository {
    pool: PgPool,
}

impl PostgresToolApprovalRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[derive(sqlx::FromRow)]
struct ToolApprovalRow {
    id: Uuid,
    message_id: Uuid,
    call_id: String,
    status: String,
    requested_at: DateTime<Utc>,
    resolved_at: Option<DateTime<Utc>>,
}

impl TryFrom<ToolApprovalRow> for ToolApproval {
    type Error = ToolApprovalRepositoryError;

    fn try_from(row: ToolApprovalRow) -> Result<Self, Self::Error> {
        let status = ToolApprovalStatus::from_db(&row.status).ok_or_else(|| {
            ToolApprovalRepositoryError::Unexpected(format!(
                "unknown tool approval status: {}",
                row.status
            ))
        })?;

        Ok(Self {
            id: row.id,
            message_id: row.message_id,
            call_id: row.call_id,
            status,
            requested_at: row.requested_at,
            resolved_at: row.resolved_at,
        })
    }
}

fn map_sqlx_error(err: sqlx::Error) -> ToolApprovalRepositoryError {
    match err {
        sqlx::Error::Database(db_err)
            if db_err.message().contains("tool_approvals_message_id_fkey") =>
        {
            ToolApprovalRepositoryError::MessageNotFound(Uuid::nil())
        }
        other => ToolApprovalRepositoryError::Unexpected(other.to_string()),
    }
}

#[async_trait]
impl ToolApprovalRepository for PostgresToolApprovalRepository {
    async fn create_pending(
        &self,
        message_id: Uuid,
        call_id: &str,
    ) -> Result<ToolApproval, ToolApprovalRepositoryError> {
        let call_id = call_id.trim();

        if call_id.is_empty() {
            return Err(ToolApprovalRepositoryError::InvalidApproval(
                "call_id must not be empty".to_string(),
            ));
        }

        let row = sqlx::query_as::<_, ToolApprovalRow>(
            r#"
            INSERT INTO tool_approvals (message_id, call_id, status)
            VALUES ($1, $2, 'pending')
            ON CONFLICT (message_id, call_id)
            DO UPDATE SET
              status = 'pending',
              requested_at = NOW(),
              resolved_at = NULL
            RETURNING id, message_id, call_id, status, requested_at, resolved_at
            "#,
        )
        .bind(message_id)
        .bind(call_id)
        .fetch_one(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        row.try_into()
    }

    async fn list(
        &self,
        status: Option<ToolApprovalStatus>,
    ) -> Result<Vec<ToolApproval>, ToolApprovalRepositoryError> {
        let rows = sqlx::query_as::<_, ToolApprovalRow>(
            r#"
            SELECT id, message_id, call_id, status, requested_at, resolved_at
            FROM tool_approvals
            WHERE ($1::TEXT IS NULL OR status = $1)
            ORDER BY requested_at DESC, id DESC
            "#,
        )
        .bind(status.map(ToolApprovalStatus::as_str))
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        rows.into_iter().map(TryInto::try_into).collect()
    }

    async fn find_by_id(
        &self,
        id: Uuid,
    ) -> Result<Option<ToolApproval>, ToolApprovalRepositoryError> {
        let row = sqlx::query_as::<_, ToolApprovalRow>(
            r#"
            SELECT id, message_id, call_id, status, requested_at, resolved_at
            FROM tool_approvals
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        row.map(TryInto::try_into).transpose()
    }

    async fn resolve(
        &self,
        id: Uuid,
        status: ToolApprovalStatus,
    ) -> Result<ToolApproval, ToolApprovalRepositoryError> {
        if status == ToolApprovalStatus::Pending {
            return Err(ToolApprovalRepositoryError::InvalidApproval(
                "approval cannot be resolved to pending".to_string(),
            ));
        }

        let row = sqlx::query_as::<_, ToolApprovalRow>(
            r#"
            UPDATE tool_approvals
            SET status = $2,
                resolved_at = NOW()
            WHERE id = $1
            RETURNING id, message_id, call_id, status, requested_at, resolved_at
            "#,
        )
        .bind(id)
        .bind(status.as_str())
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        row.ok_or(ToolApprovalRepositoryError::NotFound(id))?
            .try_into()
    }
}
