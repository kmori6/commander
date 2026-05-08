use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum TaskResultRepositoryError {
    #[error("result not found for task: {0}")]
    NotFoundByTask(Uuid),

    #[error("task not found: {0}")]
    TaskNotFound(Uuid),

    #[error("failed to access result repository: {0}")]
    Unexpected(String),
}
