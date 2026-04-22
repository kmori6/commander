use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum TokenUsageRepositoryError {
    #[error("token usage message not found: {0}")]
    MessageNotFound(Uuid),

    #[error("failed to access token usage repository: {0}")]
    Unexpected(String),
}
