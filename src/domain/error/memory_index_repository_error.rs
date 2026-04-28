use thiserror::Error;

#[derive(Debug, Error)]
pub enum MemoryIndexRepositoryError {
    #[error("failed to access memory index repository: {0}")]
    Unexpected(String),
}
