use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum JobRunRepositoryError {
    #[error("job run not found: {0}")]
    JobRunNotFound(Uuid),

    #[error("invalid job run status: {0}")]
    InvalidStatus(String),

    #[error("failed to access job run repository: {0}")]
    Unexpected(String),
}
