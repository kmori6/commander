use crate::application::error::llm_client_error::LlmClientError;
use crate::domain::error::tool_error::ToolError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AgentError {
    #[error("failed to call llm provider: {0}")]
    LlmClient(#[from] LlmClientError),

    #[error("failed to handle tool execution: {0}")]
    Tool(#[from] ToolError),

    #[error("agent exceeded maximum tool iterations: {0}")]
    MaxToolIterations(usize),
}
