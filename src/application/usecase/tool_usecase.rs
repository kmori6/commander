use crate::application::error::tool_service_error::ToolServiceError;
use crate::application::service::tool_service::ToolService;
use crate::domain::model::tool_call::{ToolPermission, ToolPermissionMode, ToolSpec};
use crate::domain::repository::tool_approval_repository::ToolApprovalRepository;
use crate::domain::repository::tool_permission_repository::ToolPermissionRepository;
use std::sync::Arc;

pub struct ToolUsecase<P, A> {
    tool_service: Arc<ToolService<P, A>>,
}

impl<P, A> ToolUsecase<P, A>
where
    P: ToolPermissionRepository,
    A: ToolApprovalRepository,
{
    pub fn new(tool_service: Arc<ToolService<P, A>>) -> Self {
        Self { tool_service }
    }

    pub fn list_tools(&self) -> Vec<ToolSpec> {
        self.tool_service.list_tools()
    }

    pub async fn list_permissions(&self) -> Result<Vec<ToolPermission>, ToolServiceError> {
        self.tool_service.list_permissions().await
    }

    pub async fn update_permission(
        &self,
        tool_name: &str,
        mode: ToolPermissionMode,
    ) -> Result<ToolPermission, ToolServiceError> {
        self.tool_service.update_permission(tool_name, mode).await
    }
}
