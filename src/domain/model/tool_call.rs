use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolSpec {
    pub name: String,
    pub description: String,
    pub parameters: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolCall {
    pub call_id: String,
    pub name: String,
    pub arguments: Value,
}

impl ToolCall {
    /// A tool call signature identifies repeated calls by tool name and arguments.
    pub fn signature(&self) -> String {
        serde_json::json!({
            "name": self.name,
            "arguments": self.arguments,
        })
        .to_string()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ToolCallOutputStatus {
    Success,
    Error,
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

    pub fn error(call_id: impl Into<String>, output: Value) -> Self {
        Self {
            call_id: call_id.into(),
            output,
            status: ToolCallOutputStatus::Error,
        }
    }
}
