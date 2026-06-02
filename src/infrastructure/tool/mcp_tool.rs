use async_trait::async_trait;
use serde_json::Value;
use std::sync::Arc;

use crate::domain::error::tool_error::ToolError;
use crate::domain::model::tool_call::ToolPermissionMode;
use crate::domain::port::tool::Tool;
use crate::infrastructure::mcp::manager::{DiscoveredMcpTool, McpManager};

pub struct McpTool {
    name: String,
    description: String,
    parameters: Value,
    manager: Arc<McpManager>,
}

impl McpTool {
    pub fn new(discovered: DiscoveredMcpTool, manager: Arc<McpManager>) -> Self {
        Self {
            name: discovered.exposed_name,
            description: discovered.description,
            parameters: discovered.parameters,
            manager,
        }
    }
}

#[async_trait]
impl Tool for McpTool {
    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn default_permission(&self) -> ToolPermissionMode {
        ToolPermissionMode::Allow
    }

    fn parameters(&self) -> Value {
        self.parameters.clone()
    }

    async fn execute(&self, arguments: Value) -> Result<Value, ToolError> {
        self.manager.call_tool(&self.name, arguments).await
    }
}
