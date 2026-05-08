use std::{collections::HashMap, sync::Arc};

use crate::domain::error::tool_executor_error::ToolExecutorError;
use crate::domain::model::tool_call::{ToolCall, ToolCallOutput, ToolPermissionMode, ToolSpec};
use crate::domain::port::tool::Tool;

#[derive(Clone, Default)]
pub struct ToolExecutor {
    tools: HashMap<String, Arc<dyn Tool>>,
}

impl ToolExecutor {
    pub fn new(tools: Vec<Arc<dyn Tool>>) -> Self {
        let tools = tools
            .into_iter()
            .map(|tool| (tool.name().to_string(), tool))
            .collect();

        Self { tools }
    }

    pub fn list_tools(&self) -> Vec<ToolSpec> {
        self.specs()
    }

    pub fn exists(&self, tool_name: &str) -> bool {
        self.tools.contains_key(tool_name)
    }

    pub fn specs(&self) -> Vec<ToolSpec> {
        self.tools.values().map(|tool| tool.spec()).collect()
    }

    pub fn default_permission(&self, tool_name: &str) -> Option<ToolPermissionMode> {
        self.tools
            .get(tool_name)
            .map(|tool| tool.default_permission())
    }

    pub async fn execute(&self, call: ToolCall) -> Result<ToolCallOutput, ToolExecutorError> {
        let tool = self
            .tools
            .get(&call.tool_name)
            .ok_or_else(|| ToolExecutorError::UnknownTool(call.tool_name.clone()))?;

        let output = tool.execute(call.arguments).await?;

        Ok(ToolCallOutput::success(call.call_id, output))
    }
}
