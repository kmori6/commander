use crate::domain::error::event_repository_error::EventRepositoryError;
use crate::domain::error::task_repository_error::TaskRepositoryError;
use crate::domain::error::token_usage_repository_error::TokenUsageRepositoryError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum TaskUsecaseError {
    #[error("failed to access task repository: {0}")]
    TaskRepository(#[from] TaskRepositoryError),

    #[error("failed to access event repository: {0}")]
    EventRepository(#[from] EventRepositoryError),

    #[error("failed to access token usage repository: {0}")]
    TokenUsageRepository(#[from] TokenUsageRepositoryError),
}
