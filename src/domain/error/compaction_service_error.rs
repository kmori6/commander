use crate::domain::error::llm_provider_error::LlmProviderError;
use crate::domain::error::message_error::MessageError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CompactionServiceError {
    #[error("failed to call llm provider while compacting messages: {0}")]
    LlmProvider(#[from] LlmProviderError),

    #[error("invalid message while compacting messages: {0}")]
    Message(#[from] MessageError),
}
