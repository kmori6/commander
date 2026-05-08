use async_trait::async_trait;

use crate::domain::error::tool_executor_error::ToolExecutorError;
use crate::domain::model::tool::{ToolCall, ToolCallOutput, ToolPermissionMode, ToolSpec};

#[async_trait]
pub trait ToolExecutor: Send + Sync {
    fn specs(&self) -> Vec<ToolSpec>;

    fn default_permission(&self, tool_name: &str) -> Option<ToolPermissionMode>;

    async fn execute(&self, call: ToolCall) -> Result<ToolCallOutput, ToolExecutorError>;
}
