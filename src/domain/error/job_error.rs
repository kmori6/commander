use crate::domain::model::job::JobStatus;
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum JobError {
    #[error("invalid job status transition: job {job_id} is {status}")]
    InvalidStatusTransition { job_id: Uuid, status: JobStatus },
}
