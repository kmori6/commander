use async_trait::async_trait;
use uuid::Uuid;

use crate::domain::error::token_usage_repository_error::TokenUsageRepositoryError;
use crate::domain::model::token_usage::{TaskTokenUsage, TokenUsage};

pub struct CreateTokenUsage {
    pub message_id: Uuid,
    pub model: String,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cache_read_tokens: i64,
    pub cache_write_tokens: i64,
}

#[async_trait]
pub trait TokenUsageRepository: Send + Sync {
    async fn save(&self, input: CreateTokenUsage) -> Result<TokenUsage, TokenUsageRepositoryError>;

    async fn summarize_for_task(
        &self,
        task_id: Uuid,
    ) -> Result<TaskTokenUsage, TokenUsageRepositoryError>;
}
