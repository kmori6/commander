use crate::domain::model::message::{MessageContent, ToolCallOutputStatus};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolPermissionMode {
    Allow,
    Ask,
    Deny,
}

impl ToolPermissionMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Allow => "allow",
            Self::Ask => "ask",
            Self::Deny => "deny",
        }
    }

    pub fn from_db(value: &str) -> Option<Self> {
        match value {
            "allow" => Some(Self::Allow),
            "ask" => Some(Self::Ask),
            "deny" => Some(Self::Deny),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tool {
    pub name: String,
    pub description: String,
    pub default_permission: ToolPermissionMode,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolPermission {
    pub id: Uuid,
    pub tool_name: String,
    pub mode: ToolPermissionMode,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolApprovalStatus {
    Pending,
    Approved,
    Rejected,
}

impl ToolApprovalStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Approved => "approved",
            Self::Rejected => "rejected",
        }
    }

    pub fn from_db(value: &str) -> Option<Self> {
        match value {
            "pending" => Some(Self::Pending),
            "approved" => Some(Self::Approved),
            "rejected" => Some(Self::Rejected),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolApproval {
    pub id: Uuid,
    pub message_id: Uuid,
    pub call_id: String,
    pub status: ToolApprovalStatus,
    pub requested_at: DateTime<Utc>,
    pub resolved_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolSpec {
    pub name: String,
    pub description: String,
    pub parameters: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolCall {
    pub call_id: String,
    pub tool_name: String,
    pub arguments: Value,
}

impl ToolCall {
    pub fn from_message_content(content: &MessageContent) -> Option<Self> {
        match content {
            MessageContent::ToolCall {
                call_id,
                tool_name,
                arguments,
            } => Some(Self {
                call_id: call_id.clone(),
                tool_name: tool_name.clone(),
                arguments: arguments.clone(),
            }),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolCallOutput {
    pub call_id: String,
    pub output: Value,
    pub status: ToolCallOutputStatus,
}

impl ToolCallOutput {
    pub fn success(call_id: impl Into<String>, output: Value) -> Self {
        Self {
            call_id: call_id.into(),
            output,
            status: ToolCallOutputStatus::Success,
        }
    }

    pub fn error(call_id: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            call_id: call_id.into(),
            output: json!({ "error": message.into() }),
            status: ToolCallOutputStatus::Error,
        }
    }

    pub fn into_message_content(self) -> MessageContent {
        MessageContent::ToolCallOutput {
            call_id: self.call_id,
            output: self.output,
            status: self.status,
        }
    }
}
