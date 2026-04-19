use crate::domain::error::agent_error::AgentError;
use crate::domain::error::chat_repository_error::ChatRepositoryError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AgentUsecaseError {
    #[error("failed to execute agent use case: {0}")]
    Agent(#[from] AgentError),

    #[error("failed to access chat repository: {0}")]
    ChatRepository(#[from] ChatRepositoryError),
}
