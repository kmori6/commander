use crate::domain::error::message_domain_error::MessageDomainError;
use thiserror::Error;
use uuid::Uuid;

impl From<MessageDomainError> for MessageRepositoryError {
    fn from(error: MessageDomainError) -> Self {
        Self::InvalidMessage(error.to_string())
    }
}

#[derive(Debug, Error)]
pub enum MessageRepositoryError {
    #[error("invalid message: {0}")]
    InvalidMessage(String),

    #[error("failed to access message repository: {0}")]
    Unexpected(String),

    #[error("task not found: {0}")]
    TaskNotFound(Uuid),
}
