use crate::domain::error::message_repository_error::MessageRepositoryError;
use crate::domain::error::session_repository_error::SessionRepositoryError;
use crate::domain::error::task_repository_error::TaskRepositoryError;
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum MessageUsecaseError {
    #[error("failed to access message repository: {0}")]
    MessageRepository(#[from] MessageRepositoryError),

    #[error("failed to access session repository: {0}")]
    SessionRepository(#[from] SessionRepositoryError),

    #[error("failed to access task repository: {0}")]
    TaskRepository(#[from] TaskRepositoryError),

    #[error("session not found: {0}")]
    SessionNotFound(Uuid),
}
