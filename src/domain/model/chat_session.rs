use crate::domain::error::chat_session_error::ChatSessionError;
use chrono::{DateTime, Utc};
use serde::Deserialize;
use uuid::Uuid;

const SESSION_TITLE_MAX_CHARS: usize = 40;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChatSessionStatus {
    Idle,
    Running,
    AwaitingApproval,
}

impl ChatSessionStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Running => "running",
            Self::AwaitingApproval => "awaiting_approval",
        }
    }

    pub fn from_db(value: &str) -> Option<Self> {
        match value {
            "idle" => Some(Self::Idle),
            "running" => Some(Self::Running),
            "awaiting_approval" => Some(Self::AwaitingApproval),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct ChatSession {
    pub id: Uuid,
    pub title: Option<String>,
    pub status: ChatSessionStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl ChatSession {
    /// The first user message can seed a stable, human-readable session title.
    pub fn title_from_first_user_message(text: &str) -> Option<String> {
        let normalized = text.split_whitespace().collect::<Vec<_>>().join(" ");
        let title = normalized
            .chars()
            .take(SESSION_TITLE_MAX_CHARS)
            .collect::<String>();

        if title.is_empty() { None } else { Some(title) }
    }

    /// A new agent turn can start only from an idle session.
    pub fn start_turn(&self) -> Result<ChatSessionStatus, ChatSessionError> {
        match self.status {
            ChatSessionStatus::Idle => Ok(ChatSessionStatus::Running),
            ChatSessionStatus::Running => Err(ChatSessionError::AlreadyRunning {
                session_id: self.id,
            }),
            ChatSessionStatus::AwaitingApproval => Err(ChatSessionError::ApprovalPending {
                session_id: self.id,
            }),
        }
    }

    /// Resolving an approval resumes the paused agent turn.
    pub fn resolve_approval(&self) -> Result<ChatSessionStatus, ChatSessionError> {
        match self.status {
            ChatSessionStatus::AwaitingApproval => Ok(ChatSessionStatus::Running),
            ChatSessionStatus::Idle => Err(ChatSessionError::ApprovalNotPending {
                session_id: self.id,
            }),
            ChatSessionStatus::Running => Err(ChatSessionError::AlreadyRunning {
                session_id: self.id,
            }),
        }
    }

    /// A running agent turn may pause while waiting for tool approval.
    pub fn await_approval(&self) -> Result<ChatSessionStatus, ChatSessionError> {
        match self.status {
            ChatSessionStatus::Running => Ok(ChatSessionStatus::AwaitingApproval),
            ChatSessionStatus::Idle => Err(ChatSessionError::TurnNotRunning {
                session_id: self.id,
            }),
            ChatSessionStatus::AwaitingApproval => Err(ChatSessionError::ApprovalPending {
                session_id: self.id,
            }),
        }
    }

    /// A running agent turn completes by returning the session to idle.
    pub fn complete_turn(&self) -> Result<ChatSessionStatus, ChatSessionError> {
        match self.status {
            ChatSessionStatus::Running => Ok(ChatSessionStatus::Idle),
            ChatSessionStatus::Idle => Err(ChatSessionError::TurnNotRunning {
                session_id: self.id,
            }),
            ChatSessionStatus::AwaitingApproval => Err(ChatSessionError::ApprovalPending {
                session_id: self.id,
            }),
        }
    }
}
