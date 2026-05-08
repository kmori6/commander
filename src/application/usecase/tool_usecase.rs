use crate::application::error::tool_usecase_error::ToolUsecaseError;
use crate::domain::model::tool::{Tool, ToolPermission, ToolPermissionMode};
use crate::domain::repository::tool_permission_repository::ToolPermissionRepository;
use crate::domain::service::tool_registry::ToolRegistry;

pub struct ToolUsecase<R> {
    tool_registry: ToolRegistry,
    tool_permission_repository: R,
}

impl<R> ToolUsecase<R>
where
    R: ToolPermissionRepository,
{
    pub fn new(tool_registry: ToolRegistry, tool_permission_repository: R) -> Self {
        Self {
            tool_registry,
            tool_permission_repository,
        }
    }

    pub fn list_tools(&self) -> Vec<Tool> {
        self.tool_registry.list()
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
        if !self.tool_registry.exists(tool_name) {
            return Err(ToolUsecaseError::ToolNotFound(tool_name.to_string()));
        }

        self.tool_permission_repository
            .upsert(tool_name, mode)
            .await
            .map_err(Into::into)
    }
}
