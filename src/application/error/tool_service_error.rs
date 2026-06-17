use crate::domain::error::tool_error::ToolError;
use crate::domain::error::tool_permission_repository_error::ToolPermissionRepositoryError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ToolServiceError {
    #[error("tool not found: {0}")]
    ToolNotFound(String),

    #[error(transparent)]
    Tool(#[from] ToolError),

    #[error("failed to access permission repository: {0}")]
    PermissionRepository(#[from] ToolPermissionRepositoryError),
}
