use async_trait::async_trait;
use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::domain::error::task_repository_error::TaskRepositoryError;
use crate::domain::model::task::{Task, TaskSourceKind, TaskStatus};

#[derive(Debug, Clone)]
pub struct CreateTask {
    pub request: String,
    pub session_id: Option<Uuid>,
    pub source_kind: TaskSourceKind,
    pub source_message_id: Option<Uuid>,
    pub source_schedule_id: Option<Uuid>,
    pub source_tool_call_id: Option<String>,
    pub subagent_profile: Option<String>,
    pub parent_task_id: Option<Uuid>,
    pub scheduled_at: Option<DateTime<Utc>>,
}

#[async_trait]
pub trait TaskRepository: Send + Sync {
    async fn create(&self, input: CreateTask) -> Result<Task, TaskRepositoryError>;

    async fn complete(&self, id: Uuid, output: String) -> Result<Task, TaskRepositoryError>;

    async fn fail(&self, id: Uuid, error: String) -> Result<Task, TaskRepositoryError>;

    async fn cancel(&self, id: Uuid) -> Result<Task, TaskRepositoryError>;

    async fn find_by_id(&self, id: Uuid) -> Result<Option<Task>, TaskRepositoryError>;

    async fn list_by_session_id(&self, session_id: Uuid) -> Result<Vec<Task>, TaskRepositoryError>;

    async fn list_recent(
        &self,
        status: Option<TaskStatus>,
        limit: usize,
    ) -> Result<Vec<Task>, TaskRepositoryError>;

    async fn claim_queued(&self, limit: usize) -> Result<Vec<Task>, TaskRepositoryError>;

    async fn requeue_running(&self) -> Result<u64, TaskRepositoryError>;

    async fn update_status(
        &self,
        id: Uuid,
        status: TaskStatus,
    ) -> Result<Task, TaskRepositoryError>;

    async fn request_cancel(&self, id: Uuid) -> Result<Task, TaskRepositoryError>;

    async fn cancel_children(&self, parent_task_id: Uuid) -> Result<u64, TaskRepositoryError>;

    async fn has_open_children(&self, parent_task_id: Uuid) -> Result<bool, TaskRepositoryError>;

    async fn list_joinable_children(&self, limit: usize) -> Result<Vec<Task>, TaskRepositoryError>;

    async fn list_by_source_schedule_id(
        &self,
        schedule_id: Uuid,
    ) -> Result<Vec<Task>, TaskRepositoryError>;

    async fn find_by_source_schedule_id_and_scheduled_at(
        &self,
        schedule_id: Uuid,
        scheduled_at: DateTime<Utc>,
    ) -> Result<Option<Task>, TaskRepositoryError>;

    async fn list_children(
        &self,
        parent_task_id: Uuid,
        status: Option<TaskStatus>,
        limit: usize,
    ) -> Result<Vec<Task>, TaskRepositoryError>;

    async fn list_child_group(
        &self,
        parent_task_id: Uuid,
        source_tool_call_id: &str,
    ) -> Result<Vec<Task>, TaskRepositoryError>;
}
