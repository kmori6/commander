use crate::domain::error::task_repository_error::TaskRepositoryError;
use crate::domain::error::watch_repository_error::WatchRepositoryError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum WatchUsecaseError {
    #[error("failed to access task repository: {0}")]
    TaskRepository(#[from] TaskRepositoryError),

    #[error("failed to access watch repository: {0}")]
    WatchRepository(#[from] WatchRepositoryError),
}
