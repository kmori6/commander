use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::domain::error::job_run_error::JobRunError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobRun {
    pub id: Uuid,
    pub job_id: Uuid,
    pub attempt: i32,
    pub status: JobRunStatus,
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
    pub error_message: Option<String>,
}

impl JobRun {
    /// A job run starts a single execution attempt for a job.
    pub fn start(job_id: Uuid, attempt: i32) -> Self {
        Self {
            id: Uuid::new_v4(),
            job_id,
            attempt,
            status: JobRunStatus::Running,
            started_at: Utc::now(),
            finished_at: None,
            error_message: None,
        }
    }

    /// A running job run finishes successfully.
    pub fn complete(&self) -> Result<Self, JobRunError> {
        if self.status != JobRunStatus::Running {
            return Err(JobRunError::InvalidStatusTransition {
                job_run_id: self.id,
                status: self.status,
            });
        }

        let mut run = self.clone();
        run.status = JobRunStatus::Completed;
        run.finished_at = Some(Utc::now());
        run.error_message = None;
        Ok(run)
    }

    /// A running job run finishes with a failure reason.
    pub fn fail(&self, reason: impl Into<String>) -> Result<Self, JobRunError> {
        if self.status != JobRunStatus::Running {
            return Err(JobRunError::InvalidStatusTransition {
                job_run_id: self.id,
                status: self.status,
            });
        }

        let mut run = self.clone();
        run.status = JobRunStatus::Failed;
        run.finished_at = Some(Utc::now());
        run.error_message = Some(reason.into());
        Ok(run)
    }

    /// A running job run stops because cancellation was requested.
    pub fn cancel(&self) -> Result<Self, JobRunError> {
        if self.status != JobRunStatus::Running {
            return Err(JobRunError::InvalidStatusTransition {
                job_run_id: self.id,
                status: self.status,
            });
        }

        let mut run = self.clone();
        run.status = JobRunStatus::Cancelled;
        run.finished_at = Some(Utc::now());
        Ok(run)
    }

    pub fn is_terminal(&self) -> bool {
        matches!(
            self.status,
            JobRunStatus::Completed | JobRunStatus::Failed | JobRunStatus::Cancelled
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobRunStatus {
    Running,
    Completed,
    Failed,
    Cancelled,
}

impl JobRunStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }

    pub fn from_db(value: &str) -> Option<Self> {
        match value {
            "running" => Some(Self::Running),
            "completed" => Some(Self::Completed),
            "failed" => Some(Self::Failed),
            "cancelled" => Some(Self::Cancelled),
            _ => None,
        }
    }
}

impl std::fmt::Display for JobRunStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn running_job_run_can_complete() {
        let run = JobRun::start(Uuid::new_v4(), 1);

        let completed = run.complete().expect("running run should complete");

        assert_eq!(completed.status, JobRunStatus::Completed);
        assert!(completed.finished_at.is_some());
        assert!(completed.error_message.is_none());
    }

    #[test]
    fn completed_job_run_cannot_fail() {
        let run = JobRun::start(Uuid::new_v4(), 1)
            .complete()
            .expect("running run should complete");

        let err = run
            .fail("late failure")
            .expect_err("completed run should not fail");

        assert_eq!(
            err,
            JobRunError::InvalidStatusTransition {
                job_run_id: run.id,
                status: JobRunStatus::Completed,
            }
        );
    }

    #[test]
    fn running_job_run_can_fail() {
        let run = JobRun::start(Uuid::new_v4(), 1);

        let failed = run.fail("agent failed").expect("running run should fail");

        assert_eq!(failed.status, JobRunStatus::Failed);
        assert_eq!(failed.error_message.as_deref(), Some("agent failed"));
        assert!(failed.finished_at.is_some());
    }

    #[test]
    fn running_job_run_can_cancel() {
        let run = JobRun::start(Uuid::new_v4(), 1);

        let cancelled = run.cancel().expect("running run should cancel");

        assert_eq!(cancelled.status, JobRunStatus::Cancelled);
        assert!(cancelled.finished_at.is_some());
    }
}
