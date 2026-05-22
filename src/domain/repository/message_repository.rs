use async_trait::async_trait;
use uuid::Uuid;

use crate::domain::error::message_repository_error::MessageRepositoryError;
use crate::domain::model::message::{Message, NewMessage, TaskUsage};

#[async_trait]
pub trait MessageRepository: Send + Sync {
    async fn save(&self, message: NewMessage) -> Result<Message, MessageRepositoryError>;

    async fn find_by_id(&self, id: Uuid) -> Result<Option<Message>, MessageRepositoryError>;

    async fn list_for_task(&self, task_id: Uuid) -> Result<Vec<Message>, MessageRepositoryError>;

    async fn list_for_session(
        &self,
        session_id: Uuid,
        until_task_id: Option<Uuid>,
    ) -> Result<Vec<Message>, MessageRepositoryError>;

    async fn task_usage(&self, task_id: Uuid) -> Result<TaskUsage, MessageRepositoryError>;
}
