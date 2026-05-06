use thiserror::Error;
use uuid::Uuid;

use crate::domain::model::job_run::JobRunStatus;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum JobRunError {
    #[error("invalid job run status transition: job_run_id={job_run_id}, status={status}")]
    InvalidStatusTransition {
        job_run_id: Uuid,
        status: JobRunStatus,
    },
}
