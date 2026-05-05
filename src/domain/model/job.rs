use chrono::{DateTime, Utc};
use uuid::Uuid;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobKind {
    General,
    Research,
    Survey,
    Digest,
    Experiment,
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
    pub fn start(&self) -> Self {
        let mut job = self.clone();
        job.status = JobStatus::Running;
        job.started_at = Some(Utc::now());
        job
    }

    /// A running or cancel-requested job finishes successfully.
    pub fn complete(&self) -> Self {
        let mut job = self.clone();
        job.status = JobStatus::Completed;
        job.finished_at = Some(Utc::now());
        job.error_message = None;
        job
    }

    /// A running job stops with a failure reason.
    pub fn fail(&self, reason: impl Into<String>) -> Self {
        let mut job = self.clone();
        job.status = JobStatus::Failed;
        job.finished_at = Some(Utc::now());
        job.error_message = Some(reason.into());
        job
    }

    /// A running job records an operator cancellation request.
    pub fn request_cancel(&self) -> Self {
        let mut job = self.clone();
        job.status = JobStatus::CancelRequested;
        job
    }

    /// A queued, running, or cancel-requested job finishes as cancelled.
    pub fn cancel(&self) -> Self {
        let mut job = self.clone();
        job.status = JobStatus::Cancelled;
        job.finished_at = Some(Utc::now());
        job
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

    pub fn from_db(value: &str) -> Option<Self> {
        match value {
            "general" | "chat" => Some(Self::General),
            "research" => Some(Self::Research),
            "survey" => Some(Self::Survey),
            "digest" => Some(Self::Digest),
            "experiment" => Some(Self::Experiment),
            _ => None,
        }
    }
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
