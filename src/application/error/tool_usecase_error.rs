use crate::domain::error::tool_error::ToolError;
use crate::domain::error::tool_execution_rule_repository_error::ToolExecutionRuleRepositoryError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ToolUsecaseError {
    #[error("tool not found: {0}")]
    ToolNotFound(String),

    #[error("failed to handle tool: {0}")]
    Tool(#[from] ToolError),

    #[error("failed to access tool execution rule repository: {0}")]
    Repository(#[from] ToolExecutionRuleRepositoryError),
}
