use uuid::Uuid;

use crate::application::error::tool_approval_usecase_error::ToolApprovalUsecaseError;
use crate::domain::model::tool_call::{ToolApproval, ToolApprovalStatus};
use crate::domain::repository::task_repository::TaskRepository;
use crate::domain::repository::tool_approval_repository::ToolApprovalRepository;

pub struct ToolApprovalUsecase<R, T> {
    tool_approval_repository: R,
    _task_repository: T,
}

impl<R, T> ToolApprovalUsecase<R, T>
where
    R: ToolApprovalRepository,
    T: TaskRepository,
{
    pub fn new(tool_approval_repository: R, task_repository: T) -> Self {
        Self {
            tool_approval_repository,
            _task_repository: task_repository,
        }
    }

    pub async fn list(
        &self,
        status: Option<ToolApprovalStatus>,
    ) -> Result<Vec<ToolApproval>, ToolApprovalUsecaseError> {
        self.tool_approval_repository
            .list(status)
            .await
            .map_err(Into::into)
    }

    pub async fn approve(&self, id: Uuid) -> Result<ToolApproval, ToolApprovalUsecaseError> {
        self.resolve_approval(id, ToolApprovalStatus::Approved)
            .await
    }

    pub async fn reject(&self, id: Uuid) -> Result<ToolApproval, ToolApprovalUsecaseError> {
        self.resolve_approval(id, ToolApprovalStatus::Rejected)
            .await
    }

    pub async fn recover_resolved_approvals(&self) -> Result<u64, ToolApprovalUsecaseError> {
        Ok(0)
    }

    async fn resolve_approval(
        &self,
        id: Uuid,
        status: ToolApprovalStatus,
    ) -> Result<ToolApproval, ToolApprovalUsecaseError> {
        self.tool_approval_repository
            .resolve(id, status)
            .await
            .map_err(Into::into)
    }
}
