use crate::domain::error::job_error::JobError;
use chrono::{DateTime, Utc};
use uuid::Uuid;

const JOB_TITLE_MAX_CHARS: usize = 60;
const DEFAULT_JOB_TITLE: &str = "Untitled job";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Job {
    pub id: Uuid,
    pub kind: JobKind,
    pub status: JobStatus,
    pub title: String,
    pub objective: String,
    pub session_id: Option<Uuid>,
    pub parent_job_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    pub error_message: Option<String>,
}

impl Job {
    pub fn new(
        kind: JobKind,
        title: impl Into<String>,
        objective: impl Into<String>,
        session_id: Option<Uuid>,
        parent_job_id: Option<Uuid>,
    ) -> Self {
        let now = Utc::now();

        Self {
            id: Uuid::new_v4(),
            kind,
            status: JobStatus::Queued,
            title: title.into(),
            objective: objective.into(),
            session_id,
            parent_job_id,
            created_at: now,
            started_at: None,
            finished_at: None,
            error_message: None,
        }
    }

    /// A queued job starts work and records when execution began.
    pub fn start(&self) -> Result<Self, JobError> {
        if self.status != JobStatus::Queued {
            return Err(JobError::InvalidStatusTransition {
                job_id: self.id,
                status: self.status,
            });
        }

        let mut job = self.clone();
        job.status = JobStatus::Running;
        job.started_at = Some(Utc::now());
        Ok(job)
    }

    /// A running or cancel-requested job finishes successfully.
    pub fn complete(&self) -> Result<Self, JobError> {
        if !matches!(self.status, JobStatus::Running | JobStatus::CancelRequested) {
            return Err(JobError::InvalidStatusTransition {
                job_id: self.id,
                status: self.status,
            });
        }

        let mut job = self.clone();
        job.status = JobStatus::Completed;
        job.finished_at = Some(Utc::now());
        job.error_message = None;
        Ok(job)
    }

    /// A running job stops with a failure reason.
    pub fn fail(&self, reason: impl Into<String>) -> Result<Self, JobError> {
        if !matches!(self.status, JobStatus::Running | JobStatus::CancelRequested) {
            return Err(JobError::InvalidStatusTransition {
                job_id: self.id,
                status: self.status,
            });
        }

        let mut job = self.clone();
        job.status = JobStatus::Failed;
        job.finished_at = Some(Utc::now());
        job.error_message = Some(reason.into());
        Ok(job)
    }

    /// Cancelling a queued job finishes it; cancelling a running job requests cooperative stop.
    pub fn cancel(&self) -> Result<Self, JobError> {
        match self.status {
            JobStatus::Queued => {
                let mut job = self.clone();
                job.status = JobStatus::Cancelled;
                job.finished_at = Some(Utc::now());
                Ok(job)
            }
            JobStatus::Running => {
                let mut job = self.clone();
                job.status = JobStatus::CancelRequested;
                Ok(job)
            }
            JobStatus::CancelRequested => Ok(self.clone()),
            _ => Err(JobError::InvalidStatusTransition {
                job_id: self.id,
                status: self.status,
            }),
        }
    }

    pub fn is_terminal(&self) -> bool {
        matches!(
            self.status,
            JobStatus::Completed | JobStatus::Failed | JobStatus::Cancelled
        )
    }

    pub fn is_active(&self) -> bool {
        !self.is_terminal()
    }

    /// A job objective can seed a stable, human-readable job title.
    pub fn title_from_objective(objective: &str) -> Option<String> {
        let normalized = objective.split_whitespace().collect::<Vec<_>>().join(" ");
        let title = normalized
            .chars()
            .take(JOB_TITLE_MAX_CHARS)
            .collect::<String>();

        if title.is_empty() { None } else { Some(title) }
    }

    /// A job title is either explicitly provided or derived from the objective.
    pub fn title_from_input(title: Option<&str>, objective: &str) -> String {
        let source = title
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or(objective);

        Self::title_from_objective(source).unwrap_or_else(|| DEFAULT_JOB_TITLE.to_string())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobKind {
    General,
    Research,
    Survey,
    Digest,
    Experiment,
}

impl JobKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::General => "general",
            Self::Research => "research",
            Self::Survey => "survey",
            Self::Digest => "digest",
            Self::Experiment => "experiment",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "general" | "chat" => Some(Self::General),
            "research" => Some(Self::Research),
            "survey" => Some(Self::Survey),
            "digest" => Some(Self::Digest),
            "experiment" => Some(Self::Experiment),
            _ => None,
        }
    }

    pub fn from_db(value: &str) -> Option<Self> {
        Self::parse(value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobStatus {
    Queued,
    Running,
    Completed,
    Failed,
    CancelRequested,
    Cancelled,
}

impl JobStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Running => "running",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::CancelRequested => "cancel_requested",
            Self::Cancelled => "cancelled",
        }
    }

    pub fn from_db(value: &str) -> Option<Self> {
        match value {
            "queued" => Some(Self::Queued),
            "running" => Some(Self::Running),
            "completed" => Some(Self::Completed),
            "failed" => Some(Self::Failed),
            "cancel_requested" => Some(Self::CancelRequested),
            "cancelled" => Some(Self::Cancelled),
            _ => None,
        }
    }
}

impl std::fmt::Display for JobStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn new_job() -> Job {
        Job::new(JobKind::General, "test job", "test objective", None, None)
    }

    #[test]
    fn queued_job_can_be_cancelled() {
        let job = new_job();

        let cancelled = job.cancel().expect("queued job should be cancellable");

        assert_eq!(cancelled.status, JobStatus::Cancelled);
        assert!(cancelled.finished_at.is_some());
    }

    #[test]
    fn completed_job_cannot_be_cancelled() {
        let job = new_job()
            .start()
            .expect("queued job should start")
            .complete()
            .expect("running job should complete");

        let err = job
            .cancel()
            .expect_err("completed job should not be cancellable");

        assert_eq!(
            err,
            JobError::InvalidStatusTransition {
                job_id: job.id,
                status: JobStatus::Completed,
            }
        );
    }

    #[test]
    fn queued_job_can_start() {
        let job = new_job();

        let running = job.start().expect("queued job should start");

        assert_eq!(running.status, JobStatus::Running);
        assert!(running.started_at.is_some());
    }

    #[test]
    fn queued_job_cannot_complete() {
        let job = new_job();

        let err = job.complete().expect_err("queued job should not complete");

        assert_eq!(
            err,
            JobError::InvalidStatusTransition {
                job_id: job.id,
                status: JobStatus::Queued,
            }
        );
    }

    #[test]
    fn explicit_job_title_takes_priority() {
        let title = Job::title_from_input(
            Some("  Write the approval persistence design  "),
            "Fallback objective",
        );

        assert_eq!(title, "Write the approval persistence design");
    }

    #[test]
    fn missing_job_title_is_derived_from_objective() {
        let title = Job::title_from_input(None, "  Design   approval persistence  ");

        assert_eq!(title, "Design approval persistence");
    }

    #[test]
    fn blank_job_title_and_objective_use_default_title() {
        let title = Job::title_from_input(Some("  "), "  ");

        assert_eq!(title, "Untitled job");
    }
}
