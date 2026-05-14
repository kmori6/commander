use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum ToolApprovalRepositoryError {
    #[error("tool approval not found: {0}")]
    NotFound(Uuid),

    #[error("task not found: {0}")]
    TaskNotFound(Uuid),

    #[error("message content not found: {0}")]
    MessageContentNotFound(Uuid),

    #[error("invalid tool approval: {0}")]
    InvalidApproval(String),

    #[error("failed to access tool approval repository: {0}")]
    Unexpected(String),
}
