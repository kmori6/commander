use thiserror::Error;

#[derive(Debug, Error)]
pub enum ToolExecutionRuleRepositoryError {
    #[error("invalid tool execution rule action: {0}")]
    InvalidAction(String),

    #[error("failed to access tool execution rule repository: {0}")]
    Unexpected(String),
}
