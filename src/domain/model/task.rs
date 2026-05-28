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

    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Cancelled)
    }

    pub fn can_cancel(self) -> bool {
        !self.is_terminal()
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
    pub fn start(&mut self, now: DateTime<Utc>) -> Result<(), String> {
        if self.status != TaskStatus::Queued {
            return Err(invalid_transition(self.status, TaskStatus::Running));
        }

        self.status = TaskStatus::Running;
        self.updated_at = now;
        self.started_at = Some(self.started_at.unwrap_or(now));
        self.finished_at = None;
        self.error = None;
        Ok(())
    }

    // status: running -> awaiting_approval
    pub fn await_approval(&mut self, now: DateTime<Utc>) -> Result<(), String> {
        if self.status != TaskStatus::Running {
            return Err(invalid_transition(
                self.status,
                TaskStatus::AwaitingApproval,
            ));
        }

        self.status = TaskStatus::AwaitingApproval;
        self.updated_at = now;
        Ok(())
    }

    // status: awaiting_approval -> queued
    pub fn resume_after_approval(&mut self, now: DateTime<Utc>) -> Result<(), String> {
        if self.status != TaskStatus::AwaitingApproval {
            return Err(invalid_transition(self.status, TaskStatus::Queued));
        }

        self.status = TaskStatus::Queued;
        self.updated_at = now;
        self.finished_at = None;
        Ok(())
    }

    // status: running -> completed
    pub fn complete(&mut self, now: DateTime<Utc>) -> Result<(), String> {
        if self.status != TaskStatus::Running {
            return Err(invalid_transition(self.status, TaskStatus::Completed));
        }

        self.status = TaskStatus::Completed;
        self.updated_at = now;
        self.started_at = Some(self.started_at.unwrap_or(now));
        self.finished_at = Some(now);
        self.error = None;
        Ok(())
    }

    // status: queued/running/awaiting_approval -> failed
    pub fn fail(&mut self, error: impl Into<String>, now: DateTime<Utc>) -> Result<(), String> {
        let error = error.into().trim().to_string();
        if error.is_empty() {
            return Err("task error must not be empty".to_string());
        }

        if self.status.is_terminal() {
            return Err(invalid_transition(self.status, TaskStatus::Failed));
        }

        self.status = TaskStatus::Failed;
        self.updated_at = now;
        self.started_at = Some(self.started_at.unwrap_or(now));
        self.finished_at = Some(now);
        self.error = Some(error);
        Ok(())
    }

    // status: queued/running/awaiting_approval -> cancelled
    pub fn cancel(&mut self, now: DateTime<Utc>) -> Result<(), String> {
        if self.status.is_terminal() {
            return Err(invalid_transition(self.status, TaskStatus::Cancelled));
        }

        self.status = TaskStatus::Cancelled;
        self.updated_at = now;
        self.finished_at = Some(now);
        self.error = None;
        Ok(())
    }

    // reset status: running -> queued
    pub fn recover_interrupted(&mut self, now: DateTime<Utc>) -> Result<(), String> {
        if self.status != TaskStatus::Running {
            return Err(invalid_transition(self.status, TaskStatus::Queued));
        }

        self.status = TaskStatus::Queued;
        self.updated_at = now;
        self.started_at = None;
        self.finished_at = None;
        self.error = None;
        Ok(())
    }
}

fn invalid_transition(from: TaskStatus, to: TaskStatus) -> String {
    format!("invalid task transition: {from:?} -> {to:?}")
}
