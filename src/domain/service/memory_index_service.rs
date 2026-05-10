use std::sync::Arc;

use crate::domain::error::memory_index_service_error::MemoryIndexServiceError;
use crate::domain::model::memory_index::{MemoryIndexChunk, MemoryIndexSearchResult};
use crate::domain::port::embedding_provider::EmbeddingProvider;
use crate::domain::repository::memory_index_repository::MemoryIndexRepository;

const DEFAULT_MAX_CHUNK_CHARS: usize = 1024;

#[derive(Clone)]
pub struct MemoryIndexService {
    embedding_provider: Arc<dyn EmbeddingProvider>,
    repository: Arc<dyn MemoryIndexRepository>,
    max_chunk_chars: usize,
}

impl MemoryIndexService {
    pub fn new(
        embedding_provider: Arc<dyn EmbeddingProvider>,
        repository: Arc<dyn MemoryIndexRepository>,
    ) -> Self {
        Self {
            embedding_provider,
            repository,
            max_chunk_chars: DEFAULT_MAX_CHUNK_CHARS,
        }
    }

    pub fn with_config(
        embedding_provider: Arc<dyn EmbeddingProvider>,
        repository: Arc<dyn MemoryIndexRepository>,
        max_chunk_chars: usize,
    ) -> Result<Self, MemoryIndexServiceError> {
        if max_chunk_chars == 0 {
            return Err(MemoryIndexServiceError::InvalidChunkSize);
        }

        Ok(Self {
            embedding_provider,
            repository,
            max_chunk_chars,
        })
    }

    pub async fn rebuild_path_index(
        &self,
        path: &str,
        content: &str,
    ) -> Result<usize, MemoryIndexServiceError> {
        let path = path.trim();
        if path.is_empty() {
            return Err(MemoryIndexServiceError::InvalidPath);
        }

        let chunks = chunk_markdown(content, self.max_chunk_chars);
        let mut index_chunks = Vec::with_capacity(chunks.len());

        for (index, chunk_content) in chunks.into_iter().enumerate() {
            let chunk_index =
                i32::try_from(index).map_err(|_| MemoryIndexServiceError::TooManyChunks)?;
            let embedding = self.embedding_provider.embed(&chunk_content).await?;

            index_chunks.push(MemoryIndexChunk {
                path: path.to_string(),
                chunk_index,
                content: chunk_content,
                embedding,
            });
        }

        let chunk_count = index_chunks.len();
        self.repository
            .rebuild_path_index(path, index_chunks)
            .await?;

        Ok(chunk_count)
    }

    pub async fn search(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<MemoryIndexSearchResult>, MemoryIndexServiceError> {
        let query = query.trim();
        if query.is_empty() {
            return Err(MemoryIndexServiceError::EmptyQuery);
        }

        if limit == 0 {
            return Ok(Vec::new());
        }

        let embedding = self.embedding_provider.embed(query).await?;
        Ok(self.repository.search(embedding, limit).await?)
    }
}

fn chunk_markdown(content: &str, max_chunk_chars: usize) -> Vec<String> {
    let blocks = markdown_blocks(content);
    let mut chunks = Vec::new();
    let mut current = String::new();

    for block in blocks {
        if block.chars().count() > max_chunk_chars {
            push_chunk(&mut chunks, &mut current);
            chunks.extend(split_long_block(&block, max_chunk_chars));
            continue;
        }

        let separator_chars = if current.is_empty() { 0 } else { 2 };
        let next_chars = current.chars().count() + separator_chars + block.chars().count();

        if !current.is_empty() && next_chars > max_chunk_chars {
            push_chunk(&mut chunks, &mut current);
        }

        if !current.is_empty() {
            current.push_str("\n\n");
        }
        current.push_str(&block);
    }

    push_chunk(&mut chunks, &mut current);
    chunks
}

fn markdown_blocks(content: &str) -> Vec<String> {
    let normalized = content.replace("\r\n", "\n");
    let mut blocks = Vec::new();
    let mut current = String::new();

    for line in normalized.lines() {
        if line.trim().is_empty() {
            push_chunk(&mut blocks, &mut current);
            continue;
        }

        if !current.is_empty() {
            current.push('\n');
        }
        current.push_str(line);
    }

    push_chunk(&mut blocks, &mut current);
    blocks
}

fn split_long_block(block: &str, max_chunk_chars: usize) -> Vec<String> {
    let mut chunks = Vec::new();
    let mut current = String::new();
    let mut count = 0;

    for ch in block.chars() {
        current.push(ch);
        count += 1;

        if count >= max_chunk_chars {
            push_chunk(&mut chunks, &mut current);
            count = 0;
        }
    }

    push_chunk(&mut chunks, &mut current);
    chunks
}

fn push_chunk(chunks: &mut Vec<String>, current: &mut String) {
    let chunk = current.trim();
    if !chunk.is_empty() {
        chunks.push(chunk.to_string());
    }
    current.clear();
}
