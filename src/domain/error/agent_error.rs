use crate::domain::error::llm_provider_error::LlmProviderError;
use crate::domain::error::tool_error::ToolError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AgentError {
    #[error("failed to call llm provider: {0}")]
    LlmProvider(#[from] LlmProviderError),

    #[error("failed to handle tool execution: {0}")]
    Tool(#[from] ToolError),

    #[error("agent exceeded maximum tool iterations: {0}")]
    MaxToolIterations(usize),
}
