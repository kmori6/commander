use crate::application::error::tool_usecase_error::ToolUsecaseError;
use crate::application::service::tool_executor::ToolExecutor;
use crate::domain::model::tool_call::{ToolPermission, ToolPermissionMode, ToolSpec};
use crate::domain::repository::tool_permission_repository::ToolPermissionRepository;
use std::sync::Arc;

pub struct ToolUsecase<R> {
    tool_executor: Arc<ToolExecutor>,
    tool_permission_repository: R,
}

impl<R> ToolUsecase<R>
where
    R: ToolPermissionRepository,
{
    pub fn new(tool_executor: Arc<ToolExecutor>, tool_permission_repository: R) -> Self {
        Self {
            tool_executor,
            tool_permission_repository,
        }
    }

    pub fn list_tools(&self) -> Vec<ToolSpec> {
        self.tool_executor.specs()
    }

    pub async fn list_permissions(&self) -> Result<Vec<ToolPermission>, ToolUsecaseError> {
        self.tool_permission_repository
            .list()
            .await
            .map_err(Into::into)
    }

    pub async fn update_permission(
        &self,
        tool_name: &str,
        mode: ToolPermissionMode,
    ) -> Result<ToolPermission, ToolUsecaseError> {
        if !self.tool_executor.exists(tool_name) {
            return Err(ToolUsecaseError::ToolNotFound(tool_name.to_string()));
        }

        self.tool_permission_repository
            .upsert(tool_name, mode)
            .await
            .map_err(Into::into)
    }
}
