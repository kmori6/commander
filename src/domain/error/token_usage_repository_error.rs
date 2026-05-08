use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum TokenUsageRepositoryError {
    #[error("task not found: {0}")]
    TaskNotFound(Uuid),

    #[error("message not found: {0}")]
    MessageNotFound(Uuid),

    #[error("invalid token usage: {0}")]
    InvalidTokenUsage(String),

    #[error("failed to access token usage repository: {0}")]
    Unexpected(String),
}
