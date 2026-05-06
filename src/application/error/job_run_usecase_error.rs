use crate::domain::error::job_error::JobError;
use crate::domain::error::job_repository_error::JobRepositoryError;
use crate::domain::error::job_run_error::JobRunError;
use crate::domain::error::job_run_repository_error::JobRunRepositoryError;
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum JobRunUsecaseError {
    #[error("job not found: {0}")]
    JobNotFound(Uuid),

    #[error("failed to access job repository: {0}")]
    JobRepository(#[from] JobRepositoryError),

    #[error("failed to access job run repository: {0}")]
    JobRunRepository(#[from] JobRunRepositoryError),

    #[error("invalid job operation: {0}")]
    Job(#[from] JobError),

    #[error("invalid job run operation: {0}")]
    JobRun(#[from] JobRunError),
}
