use std::{collections::HashMap, sync::Arc};

use crate::application::error::tool_service_error::ToolServiceError;
use crate::domain::model::tool_call::{ToolCall, ToolCallOutput, ToolSpec};
use crate::domain::port::tool::Tool;

pub struct ToolService {
    tools: HashMap<String, Arc<dyn Tool>>,
}

impl ToolService {
    pub fn new(tools: Vec<Arc<dyn Tool>>) -> Self {
        let tools = tools
            .into_iter()
            .map(|t| (t.name().to_string(), t))
            .collect();
        Self { tools }
    }

    pub fn list_tools(&self) -> Vec<ToolSpec> {
        self.tools.values().map(|t| t.spec()).collect()
    }

    pub fn specs_for(
        &self,
        allowed_tools: Option<&[String]>,
        extra_specs: impl IntoIterator<Item = ToolSpec>,
    ) -> Vec<ToolSpec> {
        let mut specs = self.list_tools();
        specs.extend(extra_specs);
        if let Some(allowed) = allowed_tools {
            specs.retain(|s| allowed.iter().any(|t| t == &s.name));
        }
        specs
    }

    pub async fn execute(&self, call: ToolCall) -> Result<ToolCallOutput, ToolServiceError> {
        let tool = self
            .tools
            .get(&call.tool_name)
            .ok_or_else(|| ToolServiceError::ToolNotFound(call.tool_name.clone()))?;
        let output = tool.execute(call.arguments).await?;
        Ok(ToolCallOutput::success(call.call_id, output))
    }
}
