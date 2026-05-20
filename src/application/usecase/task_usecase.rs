use uuid::Uuid;

use crate::application::error::task_usecase_error::TaskUsecaseError;
use crate::domain::error::task_repository_error::TaskRepositoryError;
use crate::domain::model::event::Event;
use crate::domain::model::task::{Task, TaskStatus};
use crate::domain::model::token_usage::TaskTokenUsage;
use crate::domain::repository::event_repository::EventRepository;
use crate::domain::repository::task_repository::{CreateTask, TaskRepository};
use crate::domain::repository::token_usage_repository::TokenUsageRepository;

pub struct TaskUsecase<T, E, U> {
    task_repository: T,
    event_repository: E,
    token_usage_repository: U,
}

impl<T, E, U> TaskUsecase<T, E, U>
where
    T: TaskRepository,
    E: EventRepository,
    U: TokenUsageRepository,
{
    pub fn new(task_repository: T, event_repository: E, token_usage_repository: U) -> Self {
        Self {
            task_repository,
            event_repository,
            token_usage_repository,
        }
    }

    pub async fn create(&self, request: String) -> Result<Task, TaskUsecaseError> {
        self.task_repository
            .create(CreateTask {
                request,
                session_id: None,
                source_schedule_id: None,
                scheduled_at: None,
            })
            .await
            .map_err(Into::into)
    }

    pub async fn find(&self, id: Uuid) -> Result<Option<Task>, TaskUsecaseError> {
        self.task_repository
            .find_by_id(id)
            .await
            .map_err(Into::into)
    }

    pub async fn list(
        &self,
        status: Option<TaskStatus>,
        limit: usize,
    ) -> Result<Vec<Task>, TaskUsecaseError> {
        self.task_repository
            .list_recent(status, limit)
            .await
            .map_err(Into::into)
    }

    pub async fn request_cancel(&self, id: Uuid) -> Result<Task, TaskUsecaseError> {
        let task = self
            .task_repository
            .find_by_id(id)
            .await?
            .ok_or(TaskRepositoryError::NotFound(id))?;

        match task.status {
            TaskStatus::Queued | TaskStatus::Running | TaskStatus::AwaitingApproval => {
                self.task_repository.cancel(id).await.map_err(Into::into)
            }
            TaskStatus::Completed | TaskStatus::Failed | TaskStatus::Cancelled => Ok(task),
        }
    }

    pub async fn list_events(&self, task_id: Uuid) -> Result<Vec<Event>, TaskUsecaseError> {
        if self.task_repository.find_by_id(task_id).await?.is_none() {
            return Err(TaskRepositoryError::NotFound(task_id).into());
        }

        self.event_repository
            .list_for_task(task_id)
            .await
            .map_err(Into::into)
    }

    pub async fn find_usage(&self, task_id: Uuid) -> Result<TaskTokenUsage, TaskUsecaseError> {
        if self.task_repository.find_by_id(task_id).await?.is_none() {
            return Err(TaskRepositoryError::NotFound(task_id).into());
        }

        self.token_usage_repository
            .summarize_for_task(task_id)
            .await
            .map_err(Into::into)
    }
}
