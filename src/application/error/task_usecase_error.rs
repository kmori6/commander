use crate::domain::error::event_repository_error::EventRepositoryError;
use crate::domain::error::message_repository_error::MessageRepositoryError;
use crate::domain::error::task_repository_error::TaskRepositoryError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum TaskUsecaseError {
    #[error("failed to access task repository: {0}")]
    TaskRepository(#[from] TaskRepositoryError),

    #[error("failed to access event repository: {0}")]
    EventRepository(#[from] EventRepositoryError),

    #[error("failed to access message repository: {0}")]
    MessageRepository(#[from] MessageRepositoryError),
}
