use crate::domain::error::task_repository_error::TaskRepositoryError;
use crate::domain::error::tool_approval_repository_error::ToolApprovalRepositoryError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ToolApprovalUsecaseError {
    #[error("failed to access tool approval repository: {0}")]
    ToolApprovalRepository(#[from] ToolApprovalRepositoryError),

    #[error("failed to access task repository: {0}")]
    TaskRepository(#[from] TaskRepositoryError),
}
