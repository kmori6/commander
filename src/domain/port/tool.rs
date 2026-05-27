use async_trait::async_trait;
use serde_json::Value;

use crate::domain::error::tool_service_error::ToolServiceError;
use crate::domain::model::tool_call::{ToolPermissionMode, ToolSpec};

#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &'static str;

    fn description(&self) -> &'static str;

    fn default_permission(&self) -> ToolPermissionMode;

    fn parameters(&self) -> Value;

    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: self.name().to_string(),
            description: self.description().to_string(),
            parameters: self.parameters(),
        }
    }

    async fn execute(&self, arguments: Value) -> Result<Value, ToolServiceError>;
}
