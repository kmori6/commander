use crate::application::error::schedule_usecase_error::ScheduleUsecaseError;
use crate::domain::error::watch_repository_error::WatchRepositoryError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ScheduleDaemonError {
    #[error("failed to access schedule usecase: {0}")]
    ScheduleUsecase(#[from] ScheduleUsecaseError),

    #[error("failed to access watch repository: {0}")]
    WatchRepository(#[from] WatchRepositoryError),
}
