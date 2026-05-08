use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum ToolApprovalRepositoryError {
    #[error("tool approval not found: {0}")]
    NotFound(Uuid),

    #[error("message not found: {0}")]
    MessageNotFound(Uuid),

    #[error("invalid tool approval: {0}")]
    InvalidApproval(String),

    #[error("failed to access tool approval repository: {0}")]
    Unexpected(String),
}
