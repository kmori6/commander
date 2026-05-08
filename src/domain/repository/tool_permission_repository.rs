use async_trait::async_trait;

use crate::domain::error::tool_permission_repository_error::ToolPermissionRepositoryError;
use crate::domain::model::tool::{ToolPermission, ToolPermissionMode};

#[async_trait]
pub trait ToolPermissionRepository: Send + Sync {
    async fn list(&self) -> Result<Vec<ToolPermission>, ToolPermissionRepositoryError>;

    async fn upsert(
        &self,
        tool_name: &str,
        mode: ToolPermissionMode,
    ) -> Result<ToolPermission, ToolPermissionRepositoryError>;

    async fn find_by_tool_name(
        &self,
        tool_name: &str,
    ) -> Result<Option<ToolPermission>, ToolPermissionRepositoryError>;
}
