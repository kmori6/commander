// src/domain/error/chat_repository_error.rs
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum ChatRepositoryError {
    #[error("chat session not found: {0}")]
    SessionNotFound(Uuid),

    #[error("failed to persist chat data: {0}")]
    Unexpected(String),
}
