use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum MessageRepositoryError {
    #[error("invalid message: {0}")]
    InvalidMessage(String),

    #[error("failed to access message repository: {0}")]
    Unexpected(String),

    #[error("task not found: {0}")]
    TaskNotFound(Uuid),
}
