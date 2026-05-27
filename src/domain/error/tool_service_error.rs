use thiserror::Error;

use crate::domain::error::tool_approval_repository_error::ToolApprovalRepositoryError;
use crate::domain::error::tool_permission_repository_error::ToolPermissionRepositoryError;

#[derive(Debug, Error)]
pub enum ToolServiceError {
    #[error("tool not found: {0}")]
    ToolNotFound(String),

    #[error("invalid tool arguments: {0}")]
    InvalidArguments(String),

    #[error("failed to execute tool: {0}")]
    ExecutionFailed(String),

    #[error("failed to access permission repository: {0}")]
    PermissionRepository(#[from] ToolPermissionRepositoryError),

    #[error("failed to access approval repository: {0}")]
    ApprovalRepository(#[from] ToolApprovalRepositoryError),
}
