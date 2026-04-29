use crate::domain::error::chat_repository_error::ChatRepositoryError;
use crate::domain::model::chat_session::ChatSession;
use async_trait::async_trait;
use uuid::Uuid;

#[async_trait]
pub trait ChatSessionRepository: Send + Sync {
    async fn create(&self) -> Result<ChatSession, ChatRepositoryError>;

    async fn find_by_id(&self, id: Uuid) -> Result<Option<ChatSession>, ChatRepositoryError>;

    async fn list_recent(&self, limit: usize) -> Result<Vec<ChatSession>, ChatRepositoryError>;

    async fn delete_by_id(&self, id: Uuid) -> Result<(), ChatRepositoryError>;
}
