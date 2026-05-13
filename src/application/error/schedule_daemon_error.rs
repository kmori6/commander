use crate::application::error::schedule_usecase_error::ScheduleUsecaseError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ScheduleDaemonError {
    #[error("failed to access schedule usecase: {0}")]
    ScheduleUsecase(#[from] ScheduleUsecaseError),
}
