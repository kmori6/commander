use serde::Serialize;
use serde_json::Value;

use crate::domain::model::tool_call::ToolSpec;

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
