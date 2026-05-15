use crate::application::error::schedule_usecase_error::ScheduleUsecaseError;
use crate::application::error::watch_usecase_error::WatchUsecaseError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ScheduleDaemonError {
    #[error("failed to access schedule usecase: {0}")]
    ScheduleUsecase(#[from] ScheduleUsecaseError),

    #[error("failed to access watch usecase: {0}")]
    WatchUsecase(#[from] WatchUsecaseError),
}
