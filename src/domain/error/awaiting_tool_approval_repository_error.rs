use thiserror::Error;

#[derive(Debug, Error)]
pub enum AwaitingToolApprovalRepositoryError {
    #[error("failed to access awaiting tool approval repository: {0}")]
    Unexpected(String),
}
