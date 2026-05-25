use thiserror::Error;

use crate::domain::error::tool_approval_repository_error::ToolApprovalRepositoryError;
use crate::domain::error::tool_permission_repository_error::ToolPermissionRepositoryError;

#[derive(Debug, Error)]
pub enum ToolPermitterError {
    #[error("tool not found: {0}")]
    ToolNotFound(String),

    #[error("failed to access tool permission repository: {0}")]
    ToolPermissionRepository(#[from] ToolPermissionRepositoryError),

    #[error("failed to access tool approval repository: {0}")]
    ToolApprovalRepository(#[from] ToolApprovalRepositoryError),
}
