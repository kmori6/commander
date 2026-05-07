use async_trait::async_trait;
use uuid::Uuid;

use crate::domain::error::message_repository_error::MessageRepositoryError;
use crate::domain::model::message::{Message, MessageContent, Role};

#[async_trait]
pub trait MessageRepository: Send + Sync {
    async fn save(
        &self,
        session_id: Uuid,
        role: Role,
        contents: Vec<MessageContent>,
    ) -> Result<Message, MessageRepositoryError>;

    async fn find_by_id(&self, id: Uuid) -> Result<Option<Message>, MessageRepositoryError>;

    async fn list_for_session(
        &self,
        session_id: Uuid,
    ) -> Result<Vec<Message>, MessageRepositoryError>;
}
