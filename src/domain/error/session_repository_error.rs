use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum SessionRepositoryError {
    #[error("session not found: {0}")]
    NotFound(Uuid),

    #[error("failed to access session repository: {0}")]
    Unexpected(String),
}
