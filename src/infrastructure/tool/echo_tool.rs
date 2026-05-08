use async_trait::async_trait;
use serde_json::{Value, json};

use crate::domain::error::tool_executor_error::ToolExecutorError;
use crate::domain::model::tool_call::ToolPermissionMode;
use crate::domain::port::tool::Tool;

#[derive(Debug, Clone, Default)]
pub struct EchoTool;

#[async_trait]
impl Tool for EchoTool {
    fn name(&self) -> &'static str {
        "echo"
    }

    fn description(&self) -> &'static str {
        "Return the provided text unchanged."
    }

    fn default_permission(&self) -> ToolPermissionMode {
        ToolPermissionMode::Allow
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "text": {
                    "type": "string",
                    "description": "Text to echo."
                }
            },
            "required": ["text"]
        })
    }

    async fn execute(&self, arguments: Value) -> Result<Value, ToolExecutorError> {
        Ok(arguments)
    }
}
