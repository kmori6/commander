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
            } else if message.contains("tool_approvals_message_id_fkey") {
                ToolApprovalRepositoryError::MessageNotFound(Uuid::nil())
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
        message_id: Uuid,
        call_id: &str,
    ) -> Result<ToolApproval, ToolApprovalRepositoryError> {
        let row = sqlx::query_as::<_, ToolApprovalRow>(
            r#"
            WITH tool_call_message AS (
              SELECT id
              FROM messages
              WHERE id = $2
                AND task_id = $1
                AND role = 'assistant'
                AND EXISTS (
                  SELECT 1
                  FROM jsonb_array_elements(contents) AS content(value)
                  WHERE content.value->>'type' = 'tool_call'
                    AND content.value->>'call_id' = $3
                )
            ),
            upsert AS (
              INSERT INTO tool_approvals (task_id, message_id, call_id, status)
              SELECT $1, id, $3, 'pending'
              FROM tool_call_message
              ON CONFLICT (message_id, call_id)
              DO UPDATE SET
                task_id = EXCLUDED.task_id,
                status = 'pending',
                requested_at = NOW(),
                resolved_at = NULL
              RETURNING id, task_id, message_id, call_id, status, requested_at, resolved_at
            )
            SELECT id, task_id, message_id, call_id, status, requested_at, resolved_at
            FROM upsert
            "#,
        )
        .bind(task_id)
        .bind(message_id)
        .bind(call_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        row.ok_or_else(|| {
            ToolApprovalRepositoryError::InvalidApproval(format!(
                "tool call not found for approval: {call_id}"
            ))
        })?
        .try_into()
    }

    async fn list(
        &self,
        status: Option<ToolApprovalStatus>,
    ) -> Result<Vec<ToolApproval>, ToolApprovalRepositoryError> {
        let rows = sqlx::query_as::<_, ToolApprovalRow>(
            r#"
            SELECT id, task_id, message_id, call_id, status, requested_at, resolved_at
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
            SELECT id, task_id, message_id, call_id, status, requested_at, resolved_at
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
            RETURNING id, task_id, message_id, call_id, status, requested_at, resolved_at
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
              ta.message_id,
              ta.call_id,
              ta.status,
              ta.requested_at,
              ta.resolved_at
            FROM tool_approvals ta
            WHERE ta.task_id = $1
              AND ta.status IN ('approved', 'rejected')
              AND NOT EXISTS (
                SELECT 1
                FROM messages m
                CROSS JOIN LATERAL jsonb_array_elements(m.contents) AS content(value)
                WHERE m.task_id = ta.task_id
                  AND m.role = 'user'
                  AND content.value->>'type' = 'tool_call_output'
                  AND content.value->>'call_id' = ta.call_id
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
            WHERE t.status = 'awaiting_approval'
              AND ta.status IN ('approved', 'rejected')
              AND NOT EXISTS (
                SELECT 1
                FROM messages m
                CROSS JOIN LATERAL jsonb_array_elements(m.contents) AS content(value)
                WHERE m.task_id = ta.task_id
                  AND m.role = 'user'
                  AND content.value->>'type' = 'tool_call_output'
                  AND content.value->>'call_id' = ta.call_id
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
