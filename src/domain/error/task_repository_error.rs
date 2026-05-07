use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum TaskRepositoryError {
    #[error("task not found: {0}")]
    NotFound(Uuid),

    #[error("session not found: {0}")]
    SessionNotFound(Uuid),

    #[error("message not found: {0}")]
    MessageNotFound(Uuid),

    #[error("parent task not found: {0}")]
    ParentTaskNotFound(Uuid),

    #[error("invalid task: {0}")]
    InvalidTask(String),

    #[error("failed to access task repository: {0}")]
    Unexpected(String),
}
