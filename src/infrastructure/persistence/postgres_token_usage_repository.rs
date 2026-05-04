use async_trait::async_trait;
use sqlx::PgPool;
use uuid::Uuid;

use crate::domain::error::token_usage_repository_error::TokenUsageRepositoryError;
use crate::domain::model::token_usage::TokenUsage;
use crate::domain::repository::token_usage_repository::TokenUsageRepository;

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
    input_tokens: i64,
    output_tokens: i64,
    cache_read_tokens: i64,
    cache_write_tokens: i64,
}

impl TryFrom<TokenUsageRow> for TokenUsage {
    type Error = TokenUsageRepositoryError;

    fn try_from(row: TokenUsageRow) -> Result<Self, Self::Error> {
        Ok(Self {
            input_tokens: to_u64("input_tokens", row.input_tokens)?,
            output_tokens: to_u64("output_tokens", row.output_tokens)?,
            cache_read_tokens: to_u64("cache_read_tokens", row.cache_read_tokens)?,
            cache_write_tokens: to_u64("cache_write_tokens", row.cache_write_tokens)?,
        })
    }
}

fn map_sqlx_error(err: sqlx::Error) -> TokenUsageRepositoryError {
    TokenUsageRepositoryError::Unexpected(err.to_string())
}

fn to_i64(name: &str, value: u64) -> Result<i64, TokenUsageRepositoryError> {
    i64::try_from(value).map_err(|_| {
        TokenUsageRepositoryError::Unexpected(format!("{name} exceeds BIGINT: {value}"))
    })
}

fn to_u64(name: &str, value: i64) -> Result<u64, TokenUsageRepositoryError> {
    u64::try_from(value)
        .map_err(|_| TokenUsageRepositoryError::Unexpected(format!("{name} is negative: {value}")))
}

#[async_trait]
impl TokenUsageRepository for PostgresTokenUsageRepository {
    async fn record_for_message(
        &self,
        message_id: Uuid,
        model: &str,
        usage: TokenUsage,
    ) -> Result<(), TokenUsageRepositoryError> {
        sqlx::query(
            r#"
            INSERT INTO token_usages (
              message_id, model, input_tokens, output_tokens, cache_read_tokens, cache_write_tokens
            )
            VALUES ($1, $2, $3, $4, $5, $6)
            "#,
        )
        .bind(message_id)
        .bind(model)
        .bind(to_i64("input_tokens", usage.input_tokens)?)
        .bind(to_i64("output_tokens", usage.output_tokens)?)
        .bind(to_i64("cache_read_tokens", usage.cache_read_tokens)?)
        .bind(to_i64("cache_write_tokens", usage.cache_write_tokens)?)
        .execute(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        Ok(())
    }

    async fn find_latest_for_session(
        &self,
        session_id: Uuid,
    ) -> Result<Option<TokenUsage>, TokenUsageRepositoryError> {
        let row = sqlx::query_as::<_, TokenUsageRow>(
            r#"
            SELECT
              tu.input_tokens,
              tu.output_tokens,
              tu.cache_read_tokens,
              tu.cache_write_tokens
            FROM token_usages tu
            JOIN chat_messages cm ON cm.id = tu.message_id
            WHERE cm.session_id = $1
            ORDER BY tu.created_at DESC, tu.id DESC
            LIMIT 1
            "#,
        )
        .bind(session_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        row.map(TryInto::try_into).transpose()
    }

    async fn sum_for_session(
        &self,
        session_id: Uuid,
    ) -> Result<TokenUsage, TokenUsageRepositoryError> {
        let row = sqlx::query_as::<_, TokenUsageRow>(
            r#"
        SELECT
          COALESCE(SUM(tu.input_tokens), 0)::BIGINT AS input_tokens,
          COALESCE(SUM(tu.output_tokens), 0)::BIGINT AS output_tokens,
          COALESCE(SUM(tu.cache_read_tokens), 0)::BIGINT AS cache_read_tokens,
          COALESCE(SUM(tu.cache_write_tokens), 0)::BIGINT AS cache_write_tokens
        FROM token_usages tu
        JOIN chat_messages cm ON cm.id = tu.message_id
        WHERE cm.session_id = $1
        "#,
        )
        .bind(session_id)
        .fetch_one(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        row.try_into()
    }
}
