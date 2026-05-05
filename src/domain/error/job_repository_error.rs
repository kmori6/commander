use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum JobRepositoryError {
    #[error("job not found: {0}")]
    JobNotFound(Uuid),

    #[error("invalid job kind: {0}")]
    InvalidKind(String),

    #[error("invalid job status: {0}")]
    InvalidStatus(String),

    #[error("failed to access job repository: {0}")]
    Unexpected(String),
}
