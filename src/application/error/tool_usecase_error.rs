use crate::application::error::tool_permitter_error::ToolPermitterError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ToolUsecaseError {
    #[error("failed to permit tool: {0}")]
    ToolPermitter(#[from] ToolPermitterError),
}
