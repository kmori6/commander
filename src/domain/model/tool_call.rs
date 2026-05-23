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

    pub fn without_approval(self) -> Self {
        match self {
            Self::Allow => Self::Allow,
            Self::Ask | Self::Deny => Self::Deny,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolPermission {
    pub tool_name: String,
    pub mode: ToolPermissionMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolApprovalStatus {
    Pending,
    Approved,
    Rejected,
}

impl ToolApprovalStatus {
    pub fn is_resolved(self) -> bool {
        matches!(self, Self::Approved | Self::Rejected)
    }

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
    pub task_id: Uuid,
    pub message_id: Uuid,
    pub call_id: String,
    pub status: ToolApprovalStatus,
    pub requested_at: DateTime<Utc>,
    pub resolved_at: Option<DateTime<Utc>>,
}

impl ToolApproval {
    // tool approval status: pending -> approved/rejected
    pub fn resolve(
        &mut self,
        status: ToolApprovalStatus,
        resolved_at: DateTime<Utc>,
    ) -> Result<(), String> {
        if !status.is_resolved() {
            return Err("approval cannot be resolved to pending".to_string());
        }

        if self.status.is_resolved() {
            return Err("approval is already resolved".to_string());
        }

        self.status = status;
        self.resolved_at = Some(resolved_at);
        Ok(())
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn permission_without_approval_denies_ask() {
        assert_eq!(
            ToolPermissionMode::Allow.without_approval(),
            ToolPermissionMode::Allow
        );
        assert_eq!(
            ToolPermissionMode::Ask.without_approval(),
            ToolPermissionMode::Deny
        );
        assert_eq!(
            ToolPermissionMode::Deny.without_approval(),
            ToolPermissionMode::Deny
        );
    }

    #[test]
    fn approval_is_resolved_for_final_status() {
        assert!(!ToolApprovalStatus::Pending.is_resolved());
        assert!(ToolApprovalStatus::Approved.is_resolved());
        assert!(ToolApprovalStatus::Rejected.is_resolved());
    }

    #[test]
    fn tool_call_from_message_content_extracts_call() {
        let content = MessageContent::ToolCall {
            call_id: "call_1".to_string(),
            tool_name: "shell".to_string(),
            arguments: json!({ "cmd": "pwd" }),
        };

        let call = ToolCall::from_message_content(&content).expect("tool call");

        assert_eq!(call.call_id, "call_1");
        assert_eq!(call.tool_name, "shell");
        assert_eq!(call.arguments, json!({ "cmd": "pwd" }));
    }

    #[test]
    fn tool_output_becomes_message_content() {
        let content = ToolCallOutput::error("call_1", "denied").into_message_content();

        assert_eq!(
            content,
            MessageContent::ToolCallOutput {
                call_id: "call_1".to_string(),
                output: json!({ "error": "denied" }),
                status: ToolCallOutputStatus::Error,
            }
        );
    }
}
