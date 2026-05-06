use crate::domain::error::awaiting_tool_approval_repository_error::AwaitingToolApprovalRepositoryError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ApprovalUsecaseError {
    #[error("failed to access awaiting tool approval repository: {0}")]
    AwaitingToolApprovalRepository(#[from] AwaitingToolApprovalRepositoryError),
}
