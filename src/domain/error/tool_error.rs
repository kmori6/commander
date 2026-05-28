use thiserror::Error;

#[derive(Debug, Error)]
pub enum ToolError {
    #[error("invalid tool arguments: {0}")]
    InvalidArguments(String),

    #[error("failed to execute tool: {0}")]
    ExecutionFailed(String),
}
