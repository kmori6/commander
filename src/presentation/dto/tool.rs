use serde::Serialize;
use serde_json::Value;

use crate::domain::model::tool_call::{ToolPermission, ToolSpec};

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
