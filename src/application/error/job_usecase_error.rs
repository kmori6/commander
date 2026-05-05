use crate::domain::error::job_repository_error::JobRepositoryError;
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum JobUsecaseError {
    #[error("job not found: {0}")]
    JobNotFound(Uuid),

    #[error("failed to access job repository: {0}")]
    Repository(#[from] JobRepositoryError),
}
