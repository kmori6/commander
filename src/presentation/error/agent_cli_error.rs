use crate::application::error::{
    agent_usecase_error::AgentUsecaseError, llm_client_error::LlmClientError,
};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AgentCliError {
    #[error("failed to initialize llm client: {0}")]
    LlmClient(#[from] LlmClientError),

    #[error("failed to execute agent use case: {0}")]
    Usecase(#[from] AgentUsecaseError),

    #[error("failed to read or write cli input/output: {0}")]
    Io(#[from] std::io::Error),
}
