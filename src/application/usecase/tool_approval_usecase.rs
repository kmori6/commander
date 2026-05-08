use uuid::Uuid;

use crate::application::error::tool_approval_usecase_error::ToolApprovalUsecaseError;
use crate::domain::model::tool_call::{ToolApproval, ToolApprovalStatus};
use crate::domain::repository::tool_approval_repository::ToolApprovalRepository;

pub struct ToolApprovalUsecase<R> {
    tool_approval_repository: R,
}

impl<R> ToolApprovalUsecase<R>
where
    R: ToolApprovalRepository,
{
    pub fn new(tool_approval_repository: R) -> Self {
        Self {
            tool_approval_repository,
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
        self.tool_approval_repository
            .resolve(id, ToolApprovalStatus::Approved)
            .await
            .map_err(Into::into)
    }

    pub async fn reject(&self, id: Uuid) -> Result<ToolApproval, ToolApprovalUsecaseError> {
        self.tool_approval_repository
            .resolve(id, ToolApprovalStatus::Rejected)
            .await
            .map_err(Into::into)
    }
}
