use crate::application::error::tool_usecase_error::ToolUsecaseError;
use crate::application::service::tool_executor::ToolExecutor;
use crate::application::service::tool_permitter::ToolPermitter;
use crate::domain::model::tool_call::{ToolPermission, ToolPermissionMode, ToolSpec};
use crate::domain::repository::tool_approval_repository::ToolApprovalRepository;
use crate::domain::repository::tool_permission_repository::ToolPermissionRepository;
use std::sync::Arc;

pub struct ToolUsecase<P, A> {
    tool_executor: Arc<ToolExecutor>,
    tool_permitter: Arc<ToolPermitter<P, A>>,
}

impl<R, A> ToolUsecase<R, A>
where
    R: ToolPermissionRepository,
    A: ToolApprovalRepository,
{
    pub fn new(tool_executor: Arc<ToolExecutor>, tool_permitter: Arc<ToolPermitter<R, A>>) -> Self {
        Self {
            tool_executor,
            tool_permitter,
        }
    }

    pub fn list_tools(&self) -> Vec<ToolSpec> {
        self.tool_executor.specs()
    }

    pub async fn list_permissions(&self) -> Result<Vec<ToolPermission>, ToolUsecaseError> {
        self.tool_permitter.list().await.map_err(Into::into)
    }

    pub async fn update_permission(
        &self,
        tool_name: &str,
        mode: ToolPermissionMode,
    ) -> Result<ToolPermission, ToolUsecaseError> {
        self.tool_permitter
            .update(tool_name, mode)
            .await
            .map_err(Into::into)
    }
}
