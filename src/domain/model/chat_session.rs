use crate::domain::error::chat_session_error::ChatSessionError;
use chrono::{DateTime, Utc};
use serde::Deserialize;
use uuid::Uuid;

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
    pub status: ChatSessionStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl ChatSession {
    pub fn can_start_turn(&self) -> Result<(), ChatSessionError> {
        match self.status {
            ChatSessionStatus::Idle => Ok(()),
            ChatSessionStatus::Running => Err(ChatSessionError::AlreadyRunning {
                session_id: self.id,
            }),
            ChatSessionStatus::AwaitingApproval => Err(ChatSessionError::ApprovalPending {
                session_id: self.id,
            }),
        }
    }

    pub fn can_resolve_approval(&self) -> Result<(), ChatSessionError> {
        match self.status {
            ChatSessionStatus::AwaitingApproval => Ok(()),
            ChatSessionStatus::Idle => Err(ChatSessionError::ApprovalNotPending {
                session_id: self.id,
            }),
            ChatSessionStatus::Running => Err(ChatSessionError::AlreadyRunning {
                session_id: self.id,
            }),
        }
    }

    pub fn resolve_approval(&self) -> Result<ChatSessionStatus, ChatSessionError> {
        self.can_resolve_approval()?;
        Ok(ChatSessionStatus::Running)
    }

    pub fn start_turn(&self) -> Result<ChatSessionStatus, ChatSessionError> {
        self.can_start_turn()?;
        Ok(ChatSessionStatus::Running)
    }

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
