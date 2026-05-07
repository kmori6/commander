use crate::domain::error::session_repository_error::SessionRepositoryError;
use crate::domain::error::task_repository_error::TaskRepositoryError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum TaskUsecaseError {
    #[error("failed to access task repository: {0}")]
    TaskRepository(#[from] TaskRepositoryError),

    #[error("failed to access session repository: {0}")]
    SessionRepository(#[from] SessionRepositoryError),
}
