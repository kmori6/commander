use crate::domain::error::token_usage_repository_error::TokenUsageRepositoryError;
use crate::domain::model::token_usage::TokenUsage;
use async_trait::async_trait;
use uuid::Uuid;

#[async_trait]
pub trait TokenUsageRepository: Send + Sync {
    async fn record_for_message(
        &self,
        message_id: Uuid,
        model: &str,
        usage: TokenUsage,
    ) -> Result<(), TokenUsageRepositoryError>;

    async fn find_latest_for_session(
        &self,
        session_id: Uuid,
    ) -> Result<Option<TokenUsage>, TokenUsageRepositoryError>;
}
