use crate::domain::error::message_repository_error::MessageRepositoryError;
use crate::domain::error::session_repository_error::SessionRepositoryError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum MessageUsecaseError {
    #[error("failed to access message repository: {0}")]
    MessageRepository(#[from] MessageRepositoryError),

    #[error("failed to access session repository: {0}")]
    SessionRepository(#[from] SessionRepositoryError),
}
