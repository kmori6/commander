use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum ScheduleRepositoryError {
    #[error("schedule not found: {0}")]
    NotFound(Uuid),

    #[error("invalid schedule: {0}")]
    InvalidSchedule(String),

    #[error("failed to access schedule repository: {0}")]
    Unexpected(String),
}
