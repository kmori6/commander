use crate::application::service::tool_service::ToolService;
use crate::domain::model::tool_call::ToolSpec;
use std::sync::Arc;

pub struct ToolUsecase {
    tool_service: Arc<ToolService>,
}

impl ToolUsecase {
    pub fn new(tool_service: Arc<ToolService>) -> Self {
        Self { tool_service }
    }

    pub fn list_tools(&self) -> Vec<ToolSpec> {
        self.tool_service.list_tools()
    }
}
