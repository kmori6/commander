use crate::domain::model::tool_execution_policy::ToolExecutionPolicy;
use serde::Deserialize;
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct ToolApprovalRequest {
    pub call_id: String,
    pub tool_name: String,
    pub arguments: Value,
    pub policy: ToolExecutionPolicy,
}

#[derive(Debug, Clone)]
pub struct ToolApproval {
    pub session_id: Uuid,
    pub tool_call_id: String,
    pub tool_name: String,
    pub arguments: Value,
    pub decision: ToolApprovalResponse,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolApprovalResponse {
    Approved,
    Denied,
}

impl ToolApprovalResponse {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Approved => "approved",
            Self::Denied => "denied",
        }
    }
}
