use serde::Serialize;
use serde_json::Value;

use crate::domain::model::tool_call::{ToolApproval, ToolPermission, ToolSpec};

#[derive(Debug, Serialize)]
pub struct ToolResponse {
    pub name: String,
    pub description: String,
    pub parameters: Value,
}

impl From<ToolSpec> for ToolResponse {
    fn from(tool: ToolSpec) -> Self {
        Self {
            name: tool.name,
            description: tool.description,
            parameters: tool.parameters,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct ToolPermissionResponse {
    pub tool_name: String,
    pub mode: String,
}

impl From<ToolPermission> for ToolPermissionResponse {
    fn from(permission: ToolPermission) -> Self {
        Self {
            tool_name: permission.tool_name,
            mode: permission.mode.as_str().to_string(),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct ToolApprovalResponse {
    pub id: String,
    pub task_id: String,
    pub message_id: String,
    pub call_id: String,
    pub status: String,
    pub requested_at: String,
    pub resolved_at: Option<String>,
}

impl From<ToolApproval> for ToolApprovalResponse {
    fn from(approval: ToolApproval) -> Self {
        Self {
            id: approval.id.to_string(),
            task_id: approval.task_id.to_string(),
            message_id: approval.message_id.to_string(),
            call_id: approval.call_id,
            status: approval.status.as_str().to_string(),
            requested_at: approval.requested_at.to_rfc3339(),
            resolved_at: approval.resolved_at.map(|dt| dt.to_rfc3339()),
        }
    }
}
