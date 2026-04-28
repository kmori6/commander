use crate::domain::error::embedding_provider_error::EmbeddingProviderError;
use crate::domain::error::memory_index_repository_error::MemoryIndexRepositoryError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum MemoryIndexServiceError {
    #[error("memory index path must not be empty")]
    InvalidPath,

    #[error("memory search query must not be empty")]
    EmptyQuery,

    #[error("memory chunk size must be greater than 0")]
    InvalidChunkSize,

    #[error("too many chunks for one memory document")]
    TooManyChunks,

    #[error("embedding provider failed: {0}")]
    EmbeddingProvider(#[from] EmbeddingProviderError),

    #[error("memory index repository failed: {0}")]
    Repository(#[from] MemoryIndexRepositoryError),
}
