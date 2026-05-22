use crate::domain::error::task_domain_error::TaskDomainError;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskSource {
    Direct,
    Session {
        session_id: Uuid,
    },
    Schedule {
        schedule_id: Uuid,
        scheduled_at: DateTime<Utc>,
    },
    Watch {
        scheduled_at: DateTime<Utc>,
    },
}

impl TaskSource {
    // source columns -> task source
    pub fn restore(
        session_id: Option<Uuid>,
        schedule_id: Option<Uuid>,
        scheduled_at: Option<DateTime<Utc>>,
    ) -> Result<Self, TaskDomainError> {
        match (session_id, schedule_id, scheduled_at) {
            (None, None, None) => Ok(Self::Direct),
            (Some(session_id), None, None) => Ok(Self::Session { session_id }),
            (None, Some(schedule_id), Some(scheduled_at)) => Ok(Self::Schedule {
                schedule_id,
                scheduled_at,
            }),
            (None, None, Some(scheduled_at)) => Ok(Self::Watch { scheduled_at }),
            _ => Err(TaskDomainError::InvalidSource(
                "session_id, schedule_id, and scheduled_at combination is invalid".to_string(),
            )),
        }
    }

    pub fn session_id(&self) -> Option<Uuid> {
        match self {
            Self::Session { session_id } => Some(*session_id),
            _ => None,
        }
    }

    pub fn schedule_id(&self) -> Option<Uuid> {
        match self {
            Self::Schedule { schedule_id, .. } => Some(*schedule_id),
            _ => None,
        }
    }

    pub fn scheduled_at(&self) -> Option<DateTime<Utc>> {
        match self {
            Self::Schedule { scheduled_at, .. } | Self::Watch { scheduled_at } => {
                Some(*scheduled_at)
            }
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Queued,
    Running,
    AwaitingApproval,
    Completed,
    Failed,
    Cancelled,
}

impl TaskStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Running => "running",
            Self::AwaitingApproval => "awaiting_approval",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }

    pub fn from_db(value: &str) -> Option<Self> {
        match value {
            "queued" => Some(Self::Queued),
            "running" => Some(Self::Running),
            "awaiting_approval" => Some(Self::AwaitingApproval),
            "completed" => Some(Self::Completed),
            "failed" => Some(Self::Failed),
            "cancelled" => Some(Self::Cancelled),
            _ => None,
        }
    }

    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Cancelled)
    }

    pub fn can_cancel(self) -> bool {
        !self.is_terminal()
    }

    // allowed lifecycle transitions
    pub fn can_transition_to(self, to: Self) -> bool {
        use TaskStatus::*;

        matches!(
            (self, to),
            (Queued, Running)
                | (Queued, Failed)
                | (Queued, Cancelled)
                | (Running, AwaitingApproval)
                | (Running, Completed)
                | (Running, Failed)
                | (Running, Cancelled)
                | (AwaitingApproval, Queued)
                | (AwaitingApproval, Failed)
                | (AwaitingApproval, Cancelled)
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Task {
    pub id: Uuid,
    pub source: TaskSource,
    pub status: TaskStatus,
    pub error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
}

impl Task {
    // persisted row -> domain task
    pub fn restore(
        id: Uuid,
        status: TaskStatus,
        session_id: Option<Uuid>,
        schedule_id: Option<Uuid>,
        scheduled_at: Option<DateTime<Utc>>,
        error: Option<String>,
        created_at: DateTime<Utc>,
        updated_at: DateTime<Utc>,
        started_at: Option<DateTime<Utc>>,
        finished_at: Option<DateTime<Utc>>,
    ) -> Result<Self, TaskDomainError> {
        validate_restored_state(status, error.as_deref(), started_at, finished_at)?;

        Ok(Self {
            id,
            source: TaskSource::restore(session_id, schedule_id, scheduled_at)?,
            status,
            error,
            created_at,
            updated_at,
            started_at,
            finished_at,
        })
    }

    // new task starts queued
    pub fn create(id: Uuid, source: TaskSource, now: DateTime<Utc>) -> Self {
        Self {
            id,
            source,
            status: TaskStatus::Queued,
            error: None,
            created_at: now,
            updated_at: now,
            started_at: None,
            finished_at: None,
        }
    }

    pub fn session_id(&self) -> Option<Uuid> {
        self.source.session_id()
    }

    pub fn schedule_id(&self) -> Option<Uuid> {
        self.source.schedule_id()
    }

    pub fn scheduled_at(&self) -> Option<DateTime<Utc>> {
        self.source.scheduled_at()
    }

    pub fn can_cancel(&self) -> bool {
        self.status.can_cancel()
    }

    // status: queued -> running
    pub fn start(&mut self, now: DateTime<Utc>) -> Result<(), TaskDomainError> {
        self.transition(TaskStatus::Running, now)?;
        self.started_at = Some(self.started_at.unwrap_or(now));
        self.finished_at = None;
        self.error = None;
        Ok(())
    }

    // status: running -> awaiting_approval
    pub fn await_approval(&mut self, now: DateTime<Utc>) -> Result<(), TaskDomainError> {
        self.transition(TaskStatus::AwaitingApproval, now)
    }

    // status: awaiting_approval -> queued
    pub fn resume_after_approval(&mut self, now: DateTime<Utc>) -> Result<(), TaskDomainError> {
        self.transition(TaskStatus::Queued, now)?;
        self.finished_at = None;
        Ok(())
    }

    // status: running -> completed
    pub fn complete(&mut self, now: DateTime<Utc>) -> Result<(), TaskDomainError> {
        self.transition(TaskStatus::Completed, now)?;
        self.started_at = Some(self.started_at.unwrap_or(now));
        self.finished_at = Some(now);
        self.error = None;
        Ok(())
    }

    // status: queued/running/awaiting_approval -> failed
    pub fn fail(
        &mut self,
        error: impl Into<String>,
        now: DateTime<Utc>,
    ) -> Result<(), TaskDomainError> {
        let error = error.into().trim().to_string();
        if error.is_empty() {
            return Err(TaskDomainError::EmptyError);
        }

        self.transition(TaskStatus::Failed, now)?;
        self.started_at = Some(self.started_at.unwrap_or(now));
        self.finished_at = Some(now);
        self.error = Some(error);
        Ok(())
    }

    // status: queued/running/awaiting_approval -> cancelled
    pub fn cancel(&mut self, now: DateTime<Utc>) -> Result<(), TaskDomainError> {
        self.transition(TaskStatus::Cancelled, now)?;
        self.finished_at = Some(now);
        self.error = None;
        Ok(())
    }

    // guard lifecycle transition
    fn transition(&mut self, to: TaskStatus, now: DateTime<Utc>) -> Result<(), TaskDomainError> {
        if self.status.is_terminal() {
            return Err(TaskDomainError::AlreadyTerminal(self.status));
        }

        if self.status == to || !self.status.can_transition_to(to) {
            return Err(TaskDomainError::InvalidTransition {
                from: self.status,
                to,
            });
        }

        self.status = to;
        self.updated_at = now;
        Ok(())
    }

    // reset status: running -> queued
    pub fn recover_interrupted(&mut self, now: DateTime<Utc>) -> Result<(), TaskDomainError> {
        if self.status != TaskStatus::Running {
            return Err(TaskDomainError::InvalidTransition {
                from: self.status,
                to: TaskStatus::Queued,
            });
        }

        self.status = TaskStatus::Queued;
        self.updated_at = now;
        self.started_at = None;
        self.finished_at = None;
        self.error = None;
        Ok(())
    }
}

// persisted state invariants
fn validate_restored_state(
    status: TaskStatus,
    error: Option<&str>,
    started_at: Option<DateTime<Utc>>,
    finished_at: Option<DateTime<Utc>>,
) -> Result<(), TaskDomainError> {
    if status == TaskStatus::Failed {
        if error.map(str::trim).unwrap_or_default().is_empty() {
            return Err(TaskDomainError::EmptyError);
        }
    } else if error.is_some() {
        return Err(TaskDomainError::InvalidState(
            "only failed tasks may have an error".to_string(),
        ));
    }

    if status.is_terminal() && finished_at.is_none() {
        return Err(TaskDomainError::InvalidState(
            "terminal tasks must have finished_at".to_string(),
        ));
    }

    if !status.is_terminal() && finished_at.is_some() {
        return Err(TaskDomainError::InvalidState(
            "active tasks must not have finished_at".to_string(),
        ));
    }

    if matches!(
        status,
        TaskStatus::Running
            | TaskStatus::AwaitingApproval
            | TaskStatus::Completed
            | TaskStatus::Failed
    ) && started_at.is_none()
    {
        return Err(TaskDomainError::InvalidState(
            "started tasks must have started_at".to_string(),
        ));
    }

    Ok(())
}
