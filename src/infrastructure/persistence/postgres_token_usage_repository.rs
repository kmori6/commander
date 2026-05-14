use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::domain::error::token_usage_repository_error::TokenUsageRepositoryError;
use crate::domain::model::token_usage::{TaskTokenUsage, TokenUsage};
use crate::domain::repository::token_usage_repository::{CreateTokenUsage, TokenUsageRepository};

#[derive(Clone)]
pub struct PostgresTokenUsageRepository {
    pool: PgPool,
}

impl PostgresTokenUsageRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[derive(sqlx::FromRow)]
struct TokenUsageRow {
    id: Uuid,
    message_id: Uuid,
    model: String,
    input_tokens: i64,
    output_tokens: i64,
    cache_read_tokens: i64,
    cache_write_tokens: i64,
    created_at: DateTime<Utc>,
}

#[derive(sqlx::FromRow)]
struct TaskTokenUsageRow {
    input_tokens: i64,
    output_tokens: i64,
    cache_read_tokens: i64,
    cache_write_tokens: i64,
}

impl From<TokenUsageRow> for TokenUsage {
    fn from(row: TokenUsageRow) -> Self {
        Self {
            id: row.id,
            message_id: row.message_id,
            model: row.model,
            input_tokens: row.input_tokens,
            output_tokens: row.output_tokens,
            cache_read_tokens: row.cache_read_tokens,
            cache_write_tokens: row.cache_write_tokens,
            created_at: row.created_at,
        }
    }
}

fn validate(input: &CreateTokenUsage) -> Result<(), TokenUsageRepositoryError> {
    if input.model.trim().is_empty() {
        return Err(TokenUsageRepositoryError::InvalidTokenUsage(
            "model must not be empty".to_string(),
        ));
    }

    if input.input_tokens < 0
        || input.output_tokens < 0
        || input.cache_read_tokens < 0
        || input.cache_write_tokens < 0
    {
        return Err(TokenUsageRepositoryError::InvalidTokenUsage(
            "token counts must be greater than or equal to zero".to_string(),
        ));
    }

    Ok(())
}

fn map_sqlx_error(err: sqlx::Error) -> TokenUsageRepositoryError {
    match err {
        sqlx::Error::Database(db_err) => {
            let message = db_err.message().to_string();

            if message.contains("token_usages_message_id_fkey") {
                TokenUsageRepositoryError::MessageNotFound(Uuid::nil())
            } else {
                TokenUsageRepositoryError::Unexpected(message)
            }
        }
        other => TokenUsageRepositoryError::Unexpected(other.to_string()),
    }
}

#[async_trait]
impl TokenUsageRepository for PostgresTokenUsageRepository {
    async fn save(&self, input: CreateTokenUsage) -> Result<TokenUsage, TokenUsageRepositoryError> {
        validate(&input)?;

        let row = sqlx::query_as::<_, TokenUsageRow>(
            r#"
            INSERT INTO token_usages (
              message_id,
              model,
              input_tokens,
              output_tokens,
              cache_read_tokens,
              cache_write_tokens
            )
            VALUES ($1, $2, $3, $4, $5, $6)
            RETURNING
              id,
              message_id,
              model,
              input_tokens,
              output_tokens,
              cache_read_tokens,
              cache_write_tokens,
              created_at
            "#,
        )
        .bind(input.message_id)
        .bind(input.model)
        .bind(input.input_tokens)
        .bind(input.output_tokens)
        .bind(input.cache_read_tokens)
        .bind(input.cache_write_tokens)
        .fetch_one(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        Ok(row.into())
    }

    async fn summarize_for_task(
        &self,
        task_id: Uuid,
    ) -> Result<TaskTokenUsage, TokenUsageRepositoryError> {
        let row = sqlx::query_as::<_, TaskTokenUsageRow>(
            r#"
            SELECT
              COALESCE(SUM(token_usages.input_tokens), 0)::BIGINT AS input_tokens,
              COALESCE(SUM(token_usages.output_tokens), 0)::BIGINT AS output_tokens,
              COALESCE(SUM(token_usages.cache_read_tokens), 0)::BIGINT AS cache_read_tokens,
              COALESCE(SUM(token_usages.cache_write_tokens), 0)::BIGINT AS cache_write_tokens
            FROM token_usages
            INNER JOIN messages
              ON messages.id = token_usages.message_id
            WHERE messages.task_id = $1
            "#,
        )
        .bind(task_id)
        .fetch_one(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        Ok(TaskTokenUsage {
            task_id,
            input_tokens: row.input_tokens,
            output_tokens: row.output_tokens,
            cache_read_tokens: row.cache_read_tokens,
            cache_write_tokens: row.cache_write_tokens,
        })
    }
}
