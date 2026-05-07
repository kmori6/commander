use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum MessageRepositoryError {
    #[error("session not found: {0}")]
    SessionNotFound(Uuid),

    #[error("message not found: {0}")]
    MessageNotFound(Uuid),

    #[error("invalid message: {0}")]
    InvalidMessage(String),

    #[error("failed to access message repository: {0}")]
    Unexpected(String),
}
