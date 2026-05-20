use async_trait::async_trait;
use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::domain::error::task_repository_error::TaskRepositoryError;
use crate::domain::model::task::{Task, TaskStatus};

#[async_trait]
pub trait TaskRepository: Send + Sync {
    async fn create(
        &self,
        session_id: Option<Uuid>,
        source_schedule_id: Option<Uuid>,
        scheduled_at: Option<DateTime<Utc>>,
    ) -> Result<Task, TaskRepositoryError>;

    async fn complete(&self, id: Uuid) -> Result<Task, TaskRepositoryError>;

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

    async fn list_by_source_schedule_id(
        &self,
        schedule_id: Uuid,
    ) -> Result<Vec<Task>, TaskRepositoryError>;

    async fn find_by_source_schedule_id_and_scheduled_at(
        &self,
        schedule_id: Uuid,
        scheduled_at: DateTime<Utc>,
    ) -> Result<Option<Task>, TaskRepositoryError>;
}
