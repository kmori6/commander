use crate::domain::error::schedule_repository_error::ScheduleRepositoryError;
use crate::domain::error::session_repository_error::SessionRepositoryError;
use crate::domain::error::task_repository_error::TaskRepositoryError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ScheduleUsecaseError {
    #[error("failed to access schedule repository: {0}")]
    ScheduleRepository(#[from] ScheduleRepositoryError),

    #[error("failed to access session repository: {0}")]
    SessionRepository(#[from] SessionRepositoryError),

    #[error("failed to access task repository: {0}")]
    TaskRepository(#[from] TaskRepositoryError),
}
