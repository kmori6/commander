use crate::domain::model::tool::{Tool, ToolPermissionMode};

#[derive(Debug, Clone)]
pub struct ToolRegistry {
    tools: Vec<Tool>,
}

impl ToolRegistry {
    pub fn new_mock() -> Self {
        Self {
            tools: vec![
                Tool {
                    name: "echo".to_string(),
                    description: "Return the given input.".to_string(),
                    default_permission: ToolPermissionMode::Allow,
                },
                Tool {
                    name: "memory_search".to_string(),
                    description: "Search private memory context.".to_string(),
                    default_permission: ToolPermissionMode::Allow,
                },
                Tool {
                    name: "file_write".to_string(),
                    description: "Write a file in the workspace.".to_string(),
                    default_permission: ToolPermissionMode::Ask,
                },
            ],
        }
    }

    pub fn list(&self) -> Vec<Tool> {
        self.tools.clone()
    }

    pub fn exists(&self, tool_name: &str) -> bool {
        self.tools.iter().any(|tool| tool.name == tool_name)
    }
}
