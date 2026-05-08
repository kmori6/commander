use async_trait::async_trait;
use uuid::Uuid;

use crate::domain::error::task_repository_error::TaskRepositoryError;
use crate::domain::model::task::{Task, TaskStatus};

#[derive(Debug, Clone)]
pub struct CreateTask {
    pub request: String,
    pub session_id: Uuid,
    pub source_message_id: Option<Uuid>,
    pub parent_task_id: Option<Uuid>,
}

#[async_trait]
pub trait TaskRepository: Send + Sync {
    async fn create(&self, input: CreateTask) -> Result<Task, TaskRepositoryError>;

    async fn find_by_id(&self, id: Uuid) -> Result<Option<Task>, TaskRepositoryError>;

    async fn list_recent(
        &self,
        status: Option<TaskStatus>,
        limit: usize,
    ) -> Result<Vec<Task>, TaskRepositoryError>;

    async fn update_status(
        &self,
        id: Uuid,
        status: TaskStatus,
    ) -> Result<Task, TaskRepositoryError>;

    async fn request_cancel(&self, id: Uuid) -> Result<Task, TaskRepositoryError>;

    async fn find_by_session_id(
        &self,
        session_id: Uuid,
    ) -> Result<Option<Task>, TaskRepositoryError>;
}
