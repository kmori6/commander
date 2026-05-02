use async_trait::async_trait;
use sqlx::PgPool;
use uuid::Uuid;

use crate::domain::error::awaiting_tool_approval_repository_error::AwaitingToolApprovalRepositoryError;
use crate::domain::model::awaiting_tool_approval::AwaitingToolApproval;
use crate::domain::repository::awaiting_tool_approval_repository::AwaitingToolApprovalRepository;

#[derive(Clone)]
pub struct PostgresAwaitingToolApprovalRepository {
    pool: PgPool,
}

impl PostgresAwaitingToolApprovalRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[derive(sqlx::FromRow)]
struct AwaitingToolApprovalRow {
    session_id: Uuid,
    assistant_message_id: Uuid,
    tool_call_id: String,
}

impl From<AwaitingToolApprovalRow> for AwaitingToolApproval {
    fn from(row: AwaitingToolApprovalRow) -> Self {
        Self {
            session_id: row.session_id,
            assistant_message_id: row.assistant_message_id,
            tool_call_id: row.tool_call_id,
        }
    }
}

fn map_sqlx_error(err: sqlx::Error) -> AwaitingToolApprovalRepositoryError {
    AwaitingToolApprovalRepositoryError::Unexpected(err.to_string())
}

#[async_trait]
impl AwaitingToolApprovalRepository for PostgresAwaitingToolApprovalRepository {
    async fn save(
        &self,
        approval: AwaitingToolApproval,
    ) -> Result<(), AwaitingToolApprovalRepositoryError> {
        sqlx::query(
            r#"
            INSERT INTO awaiting_tool_approvals (
              session_id, assistant_message_id, tool_call_id
            )
            VALUES ($1, $2, $3)
            "#,
        )
        .bind(approval.session_id)
        .bind(approval.assistant_message_id)
        .bind(approval.tool_call_id)
        .execute(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        Ok(())
    }

    async fn find_by_session_id(
        &self,
        session_id: Uuid,
    ) -> Result<Option<AwaitingToolApproval>, AwaitingToolApprovalRepositoryError> {
        let row = sqlx::query_as::<_, AwaitingToolApprovalRow>(
            r#"
            SELECT session_id, assistant_message_id, tool_call_id
            FROM awaiting_tool_approvals
            WHERE session_id = $1
            "#,
        )
        .bind(session_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        Ok(row.map(Into::into))
    }

    async fn list_all(
        &self,
    ) -> Result<Vec<AwaitingToolApproval>, AwaitingToolApprovalRepositoryError> {
        let rows = sqlx::query_as::<_, AwaitingToolApprovalRow>(
            r#"
            SELECT session_id, assistant_message_id, tool_call_id
            FROM awaiting_tool_approvals
            ORDER BY created_at DESC
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    async fn delete_by_session_id(
        &self,
        session_id: Uuid,
    ) -> Result<(), AwaitingToolApprovalRepositoryError> {
        sqlx::query(
            r#"
            DELETE FROM awaiting_tool_approvals
            WHERE session_id = $1
            "#,
        )
        .bind(session_id)
        .execute(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        Ok(())
    }
}
