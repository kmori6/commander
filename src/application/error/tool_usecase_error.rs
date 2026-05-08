use crate::domain::error::tool_permission_repository_error::ToolPermissionRepositoryError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ToolUsecaseError {
    #[error("tool not found: {0}")]
    ToolNotFound(String),

    #[error("failed to access tool permission repository: {0}")]
    ToolPermissionRepository(#[from] ToolPermissionRepositoryError),
}
