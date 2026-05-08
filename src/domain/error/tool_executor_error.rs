use thiserror::Error;

#[derive(Debug, Error)]
pub enum ToolExecutorError {
    #[error("unknown tool: {0}")]
    UnknownTool(String),

    #[error("invalid tool arguments: {0}")]
    InvalidArguments(String),

    #[error("failed to execute tool: {0}")]
    ExecutionFailed(String),
}
