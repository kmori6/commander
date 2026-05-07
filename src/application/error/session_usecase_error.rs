use crate::domain::error::session_repository_error::SessionRepositoryError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SessionUsecaseError {
    #[error("failed to access session repository: {0}")]
    SessionRepository(#[from] SessionRepositoryError),
}
