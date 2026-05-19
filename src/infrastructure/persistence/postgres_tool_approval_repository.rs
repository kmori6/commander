use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::domain::error::tool_approval_repository_error::ToolApprovalRepositoryError;
use crate::domain::model::tool_call::{ToolApproval, ToolApprovalStatus};
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
    task_id: Uuid,
    message_content_id: Uuid,
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
            task_id: row.task_id,
            message_content_id: row.message_content_id,
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
        sqlx::Error::Database(db_err) => {
            let message = db_err.message().to_string();

            if message.contains("tool_approvals_task_id_fkey") {
                ToolApprovalRepositoryError::TaskNotFound(Uuid::nil())
            } else if message.contains("tool_approvals_message_content_id_fkey") {
                ToolApprovalRepositoryError::MessageContentNotFound(Uuid::nil())
            } else {
                ToolApprovalRepositoryError::Unexpected(message)
            }
        }
        other => ToolApprovalRepositoryError::Unexpected(other.to_string()),
    }
}

#[async_trait]
impl ToolApprovalRepository for PostgresToolApprovalRepository {
    async fn create_pending(
        &self,
        task_id: Uuid,
        message_content_id: Uuid,
    ) -> Result<ToolApproval, ToolApprovalRepositoryError> {
        let row = sqlx::query_as::<_, ToolApprovalRow>(
            r#"
            WITH tool_call_content AS (
              SELECT id, message_id, call_id
              FROM message_contents
              WHERE id = $2
                AND type = 'tool_call'
            ),
            upsert AS (
              INSERT INTO tool_approvals (task_id, message_content_id, status)
              SELECT $1, id, 'pending'
              FROM tool_call_content
              ON CONFLICT (message_content_id)
              DO UPDATE SET
                task_id = EXCLUDED.task_id,
                status = 'pending',
                requested_at = NOW(),
                resolved_at = NULL
              RETURNING id, task_id, message_content_id, status, requested_at, resolved_at
            )
            SELECT
              upsert.id,
              upsert.task_id,
              upsert.message_content_id,
              tool_call_content.message_id,
              COALESCE(tool_call_content.call_id, '') AS call_id,
              upsert.status,
              upsert.requested_at,
              upsert.resolved_at
            FROM upsert
            INNER JOIN tool_call_content
              ON tool_call_content.id = upsert.message_content_id
            "#,
        )
        .bind(task_id)
        .bind(message_content_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        row.ok_or(ToolApprovalRepositoryError::MessageContentNotFound(
            message_content_id,
        ))?
        .try_into()
    }

    async fn list(
        &self,
        status: Option<ToolApprovalStatus>,
    ) -> Result<Vec<ToolApproval>, ToolApprovalRepositoryError> {
        let rows = sqlx::query_as::<_, ToolApprovalRow>(
            r#"
            SELECT
              tool_approvals.id,
              tool_approvals.task_id,
              tool_approvals.message_content_id,
              message_contents.message_id,
              COALESCE(message_contents.call_id, '') AS call_id,
              tool_approvals.status,
              tool_approvals.requested_at,
              tool_approvals.resolved_at
            FROM tool_approvals
            INNER JOIN message_contents
              ON message_contents.id = tool_approvals.message_content_id
            WHERE ($1::TEXT IS NULL OR tool_approvals.status = $1)
            ORDER BY tool_approvals.requested_at DESC, tool_approvals.id DESC
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
            SELECT
              tool_approvals.id,
              tool_approvals.task_id,
              tool_approvals.message_content_id,
              message_contents.message_id,
              COALESCE(message_contents.call_id, '') AS call_id,
              tool_approvals.status,
              tool_approvals.requested_at,
              tool_approvals.resolved_at
            FROM tool_approvals
            INNER JOIN message_contents
              ON message_contents.id = tool_approvals.message_content_id
            WHERE tool_approvals.id = $1
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
            WITH updated AS (
              UPDATE tool_approvals
              SET status = $2,
                  resolved_at = NOW()
              WHERE id = $1
              RETURNING id, task_id, message_content_id, status, requested_at, resolved_at
            )
            SELECT
              updated.id,
              updated.task_id,
              updated.message_content_id,
              message_contents.message_id,
              COALESCE(message_contents.call_id, '') AS call_id,
              updated.status,
              updated.requested_at,
              updated.resolved_at
            FROM updated
            INNER JOIN message_contents
              ON message_contents.id = updated.message_content_id
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

    async fn ready_for_task(
        &self,
        task_id: Uuid,
    ) -> Result<Vec<ToolApproval>, ToolApprovalRepositoryError> {
        let rows = sqlx::query_as::<_, ToolApprovalRow>(
            r#"
            SELECT
              ta.id,
              ta.task_id,
              ta.message_content_id,
              mc.message_id,
              COALESCE(mc.call_id, '') AS call_id,
              ta.status,
              ta.requested_at,
              ta.resolved_at
            FROM tool_approvals ta
            INNER JOIN message_contents mc
              ON mc.id = ta.message_content_id
            WHERE ta.task_id = $1
              AND ta.status IN ('approved', 'rejected')
              AND NOT EXISTS (
                SELECT 1
                FROM messages m
                INNER JOIN message_contents out_content
                  ON out_content.message_id = m.id
                WHERE m.task_id = ta.task_id
                  AND m.role = 'user'
                  AND out_content.type = 'tool_call_output'
                  AND out_content.call_id = mc.call_id
              )
            ORDER BY ta.resolved_at ASC NULLS LAST, ta.requested_at ASC, ta.id ASC
            "#,
        )
        .bind(task_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        rows.into_iter().map(TryInto::try_into).collect()
    }

    async fn ready_task_ids(&self, limit: usize) -> Result<Vec<Uuid>, ToolApprovalRepositoryError> {
        let limit = i64::try_from(limit).map_err(|_| {
            ToolApprovalRepositoryError::Unexpected(format!("invalid limit: {limit}"))
        })?;

        sqlx::query_scalar::<_, Uuid>(
            r#"
            SELECT ta.task_id
            FROM tool_approvals ta
            INNER JOIN tasks t
              ON t.id = ta.task_id
            INNER JOIN message_contents mc
              ON mc.id = ta.message_content_id
            WHERE t.status = 'awaiting_approval'
              AND ta.status IN ('approved', 'rejected')
              AND NOT EXISTS (
                SELECT 1
                FROM messages m
                INNER JOIN message_contents out_content
                  ON out_content.message_id = m.id
                WHERE m.task_id = ta.task_id
                  AND m.role = 'user'
                  AND out_content.type = 'tool_call_output'
                  AND out_content.call_id = mc.call_id
            )
            GROUP BY ta.task_id
            ORDER BY MIN(ta.resolved_at) ASC NULLS LAST,
                     MIN(ta.requested_at) ASC,
                     ta.task_id ASC
            LIMIT $1
            "#,
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)
    }
}
