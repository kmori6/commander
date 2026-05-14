use async_trait::async_trait;
use uuid::Uuid;

use crate::domain::error::tool_approval_repository_error::ToolApprovalRepositoryError;
use crate::domain::model::tool_call::{ToolApproval, ToolApprovalStatus};

#[async_trait]
pub trait ToolApprovalRepository: Send + Sync {
    async fn create_pending(
        &self,
        task_id: Uuid,
        message_content_id: Uuid,
    ) -> Result<ToolApproval, ToolApprovalRepositoryError>;

    async fn list(
        &self,
        status: Option<ToolApprovalStatus>,
    ) -> Result<Vec<ToolApproval>, ToolApprovalRepositoryError>;

    async fn find_by_id(
        &self,
        id: Uuid,
    ) -> Result<Option<ToolApproval>, ToolApprovalRepositoryError>;

    async fn resolve(
        &self,
        id: Uuid,
        status: ToolApprovalStatus,
    ) -> Result<ToolApproval, ToolApprovalRepositoryError>;
}
