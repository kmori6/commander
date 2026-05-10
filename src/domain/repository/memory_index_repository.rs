use crate::domain::error::memory_index_repository_error::MemoryIndexRepositoryError;
use crate::domain::model::memory_index::{MemoryIndexChunk, MemoryIndexSearchResult};
use async_trait::async_trait;

#[async_trait]
pub trait MemoryIndexRepository: Send + Sync {
    async fn rebuild_path_index(
        &self,
        path: &str,
        chunks: Vec<MemoryIndexChunk>,
    ) -> Result<(), MemoryIndexRepositoryError>;

    async fn search(
        &self,
        embedding: Vec<f32>,
        limit: usize,
    ) -> Result<Vec<MemoryIndexSearchResult>, MemoryIndexRepositoryError>;
}
