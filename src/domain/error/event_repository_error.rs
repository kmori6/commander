use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum EventRepositoryError {
    #[error("task not found: {0}")]
    TaskNotFound(Uuid),

    #[error("failed to access event repository: {0}")]
    Unexpected(String),
}
