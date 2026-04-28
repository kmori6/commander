use async_trait::async_trait;
use pgvector::Vector;
use sqlx::PgPool;

use crate::domain::error::memory_index_repository_error::MemoryIndexRepositoryError;
use crate::domain::model::memory_index::{MemoryIndexChunk, MemoryIndexSearchResult};
use crate::domain::repository::memory_index_repository::MemoryIndexRepository;

#[derive(Clone)]
pub struct PostgresMemoryIndexRepository {
    pool: PgPool,
}

impl PostgresMemoryIndexRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[derive(sqlx::FromRow)]
struct MemoryIndexSearchResultRow {
    path: String,
    chunk_index: i32,
    content: String,
    distance: f64,
}

impl From<MemoryIndexSearchResultRow> for MemoryIndexSearchResult {
    fn from(row: MemoryIndexSearchResultRow) -> Self {
        Self {
            path: row.path,
            chunk_index: row.chunk_index,
            content: row.content,
            distance: row.distance,
        }
    }
}

fn map_sqlx_error(err: sqlx::Error) -> MemoryIndexRepositoryError {
    MemoryIndexRepositoryError::Unexpected(err.to_string())
}

#[async_trait]
impl MemoryIndexRepository for PostgresMemoryIndexRepository {
    async fn rebuild_path_index(
        &self,
        path: &str,
        chunks: Vec<MemoryIndexChunk>,
    ) -> Result<(), MemoryIndexRepositoryError> {
        let mut tx = self.pool.begin().await.map_err(map_sqlx_error)?;

        sqlx::query(
            r#"
            DELETE FROM memory_index
            WHERE path = $1
            "#,
        )
        .bind(path)
        .execute(&mut *tx)
        .await
        .map_err(map_sqlx_error)?;

        for chunk in chunks {
            sqlx::query(
                r#"
                INSERT INTO memory_index (path, chunk_index, content, embedding)
                VALUES ($1, $2, $3, $4)
                "#,
            )
            .bind(chunk.path)
            .bind(chunk.chunk_index)
            .bind(chunk.content)
            .bind(Vector::from(chunk.embedding))
            .execute(&mut *tx)
            .await
            .map_err(map_sqlx_error)?;
        }

        tx.commit().await.map_err(map_sqlx_error)?;
        Ok(())
    }

    async fn search(
        &self,
        embedding: Vec<f32>,
        limit: usize,
    ) -> Result<Vec<MemoryIndexSearchResult>, MemoryIndexRepositoryError> {
        let limit = i64::try_from(limit).unwrap_or(i64::MAX);

        let rows = sqlx::query_as::<_, MemoryIndexSearchResultRow>(
            r#"
            SELECT
              path,
              chunk_index,
              content,
              embedding <=> $1 AS distance
            FROM memory_index
            ORDER BY embedding <=> $1
            LIMIT $2
            "#,
        )
        .bind(Vector::from(embedding))
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        Ok(rows.into_iter().map(Into::into).collect())
    }
}
