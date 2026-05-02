use crate::domain::error::awaiting_tool_approval_repository_error::AwaitingToolApprovalRepositoryError;
use crate::domain::model::awaiting_tool_approval::AwaitingToolApproval;
use async_trait::async_trait;
use uuid::Uuid;

#[async_trait]
pub trait AwaitingToolApprovalRepository: Send + Sync {
    async fn save(
        &self,
        approval: AwaitingToolApproval,
    ) -> Result<(), AwaitingToolApprovalRepositoryError>;

    async fn find_by_session_id(
        &self,
        session_id: Uuid,
    ) -> Result<Option<AwaitingToolApproval>, AwaitingToolApprovalRepositoryError>;

    async fn list_all(
        &self,
    ) -> Result<Vec<AwaitingToolApproval>, AwaitingToolApprovalRepositoryError>;

    async fn delete_by_session_id(
        &self,
        session_id: Uuid,
    ) -> Result<(), AwaitingToolApprovalRepositoryError>;
}
