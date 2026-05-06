use crate::domain::error::chat_repository_error::ChatRepositoryError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ChatSessionUsecaseError {
    #[error("failed to access chat repository: {0}")]
    ChatRepository(#[from] ChatRepositoryError),
}
