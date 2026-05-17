use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskSourceKind {
    Chat,
    Schedule,
    Task,
    Manual,
    Watch,
}

impl TaskSourceKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Chat => "chat",
            Self::Schedule => "schedule",
            Self::Task => "task",
            Self::Manual => "manual",
            Self::Watch => "watch",
        }
    }

    pub fn from_db(value: &str) -> Option<Self> {
        match value {
            "chat" => Some(Self::Chat),
            "schedule" => Some(Self::Schedule),
            "task" => Some(Self::Task),
            "manual" => Some(Self::Manual),
            "watch" => Some(Self::Watch),
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
    CancelRequested,
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
            Self::CancelRequested => "cancel_requested",
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
            "cancel_requested" => Some(Self::CancelRequested),
            "cancelled" => Some(Self::Cancelled),
            _ => None,
        }
    }

    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Cancelled)
    }

    pub fn can_request_cancel(self) -> bool {
        !self.is_terminal() && !matches!(self, Self::CancelRequested)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Task {
    pub id: Uuid,
    pub request: String,
    pub status: TaskStatus,
    pub session_id: Option<Uuid>,
    pub source_kind: TaskSourceKind,
    pub source_message_id: Option<Uuid>,
    pub source_schedule_id: Option<Uuid>,
    pub parent_task_id: Option<Uuid>,
    pub scheduled_at: Option<DateTime<Utc>>,
    pub output: String,
    pub error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
}

impl Task {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: Uuid,
        request: String,
        status: TaskStatus,
        session_id: Option<Uuid>,
        source_kind: TaskSourceKind,
        source_message_id: Option<Uuid>,
        source_schedule_id: Option<Uuid>,
        parent_task_id: Option<Uuid>,
        scheduled_at: Option<DateTime<Utc>>,
        output: String,
        error: Option<String>,
        created_at: DateTime<Utc>,
        updated_at: DateTime<Utc>,
        started_at: Option<DateTime<Utc>>,
        finished_at: Option<DateTime<Utc>>,
    ) -> Self {
        Self {
            id,
            request,
            status,
            session_id,
            source_kind,
            source_message_id,
            source_schedule_id,
            parent_task_id,
            scheduled_at,
            output,
            error,
            created_at,
            updated_at,
            started_at,
            finished_at,
        }
    }
}
