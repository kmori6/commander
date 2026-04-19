// src/domain/port/chat_message_repository.rs
use crate::domain::error::chat_repository_error::ChatRepositoryError;
use crate::domain::model::chat_message::ChatMessage;
use crate::domain::model::message::Message;
use async_trait::async_trait;
use uuid::Uuid;

#[async_trait]
pub trait ChatMessageRepository: Send + Sync {
    /// Append one message to a session.
    /// Implementations should also refresh the owning session's `updated_at`
    /// in the same transaction.
    async fn append(
        &self,
        session_id: Uuid,
        message: Message,
    ) -> Result<ChatMessage, ChatRepositoryError>;

    /// Return messages in ascending conversation order.
    async fn list_for_session(
        &self,
        session_id: Uuid,
    ) -> Result<Vec<ChatMessage>, ChatRepositoryError>;
}
