use uuid::Uuid;

use crate::application::error::task_usecase_error::TaskUsecaseError;
use crate::domain::error::task_repository_error::TaskRepositoryError;
use crate::domain::model::event::Event;
use crate::domain::model::task::{Task, TaskSourceKind, TaskStatus};
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

    pub async fn create(
        &self,
        request: String,
        parent_task_id: Option<Uuid>,
    ) -> Result<Task, TaskUsecaseError> {
        if let Some(parent_task_id) = parent_task_id
            && self
                .task_repository
                .find_by_id(parent_task_id)
                .await?
                .is_none()
        {
            return Err(TaskRepositoryError::ParentTaskNotFound(parent_task_id).into());
        }

        self.task_repository
            .create(CreateTask {
                request,
                session_id: None,
                source_kind: if parent_task_id.is_some() {
                    TaskSourceKind::Task
                } else {
                    TaskSourceKind::Manual
                },
                source_message_id: None,
                source_schedule_id: None,
                source_tool_call_id: None,
                subagent_profile: None,
                parent_task_id,
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
        parent_task_id: Option<Uuid>,
        limit: usize,
    ) -> Result<Vec<Task>, TaskUsecaseError> {
        if let Some(parent_task_id) = parent_task_id {
            return self
                .task_repository
                .list_children(parent_task_id, status, limit)
                .await
                .map_err(Into::into);
        }

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
            TaskStatus::Queued | TaskStatus::AwaitingApproval => {
                self.task_repository.cancel(id).await.map_err(Into::into)
            }
            TaskStatus::Running => self
                .task_repository
                .request_cancel(id)
                .await
                .map_err(Into::into),
            TaskStatus::AwaitingChild => {
                let task = self.task_repository.request_cancel(id).await?;
                self.task_repository.cancel_children(id).await?;

                if !self.task_repository.has_open_children(id).await? {
                    return self.task_repository.cancel(id).await.map_err(Into::into);
                }

                Ok(task)
            }
            TaskStatus::CancelRequested
            | TaskStatus::Completed
            | TaskStatus::Failed
            | TaskStatus::Cancelled => Ok(task),
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
