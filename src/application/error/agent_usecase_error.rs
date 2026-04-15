use crate::application::error::llm_client_error::LlmClientError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AgentUsecaseError {
    #[error("failed to execute agent use case: {0}")]
    LlmClient(#[from] LlmClientError),
}
