use crate::domain::error::tool_error::ToolError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ToolServiceError {
    #[error("tool not found: {0}")]
    ToolNotFound(String),

    #[error(transparent)]
    Tool(#[from] ToolError),
}
