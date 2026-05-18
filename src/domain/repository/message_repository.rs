use async_trait::async_trait;
use uuid::Uuid;

use crate::domain::error::message_repository_error::MessageRepositoryError;
use crate::domain::model::message::{Message, MessageContent, Role};

#[async_trait]
pub trait MessageRepository: Send + Sync {
    async fn save(
        &self,
        task_id: Uuid,
        role: Role,
        contents: Vec<MessageContent>,
    ) -> Result<Message, MessageRepositoryError>;

    async fn find_by_id(&self, id: Uuid) -> Result<Option<Message>, MessageRepositoryError>;

    async fn list_for_task(&self, task_id: Uuid) -> Result<Vec<Message>, MessageRepositoryError>;

    async fn find_tool_call_content_id(
        &self,
        message_id: Uuid,
        call_id: &str,
    ) -> Result<Option<Uuid>, MessageRepositoryError>;

    async fn has_tool_output(
        &self,
        task_id: Uuid,
        call_id: &str,
    ) -> Result<bool, MessageRepositoryError>;
}
