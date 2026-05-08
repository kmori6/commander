use uuid::Uuid;

use crate::application::error::task_usecase_error::TaskUsecaseError;
use crate::domain::error::task_repository_error::TaskRepositoryError;
use crate::domain::model::event::Event;
use crate::domain::model::session::SessionKind;
use crate::domain::model::task::{Task, TaskStatus};
use crate::domain::model::task_result::TaskResult;
use crate::domain::model::token_usage::TaskTokenUsage;
use crate::domain::repository::event_repository::EventRepository;
use crate::domain::repository::session_repository::SessionRepository;
use crate::domain::repository::task_repository::{CreateTask, TaskRepository};
use crate::domain::repository::task_result_repository::TaskResultRepository;
use crate::domain::repository::token_usage_repository::TokenUsageRepository;

pub struct TaskUsecase<T, S, R, E, U> {
    task_repository: T,
    session_repository: S,
    task_result_repository: R,
    event_repository: E,
    token_usage_repository: U,
}

impl<T, S, R, E, U> TaskUsecase<T, S, R, E, U>
where
    T: TaskRepository,
    S: SessionRepository,
    R: TaskResultRepository,
    E: EventRepository,
    U: TokenUsageRepository,
{
    pub fn new(
        task_repository: T,
        session_repository: S,
        task_result_repository: R,
        event_repository: E,
        token_usage_repository: U,
    ) -> Self {
        Self {
            task_repository,
            session_repository,
            task_result_repository,
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

        let task_session = self
            .session_repository
            .create(SessionKind::Task, Some(request.clone()))
            .await?;

        self.task_repository
            .create(CreateTask {
                request,
                session_id: task_session.id,
                source_message_id: None,
                parent_task_id,
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
        self.task_repository
            .request_cancel(id)
            .await
            .map_err(Into::into)
    }

    pub async fn find_result(&self, task_id: Uuid) -> Result<Option<TaskResult>, TaskUsecaseError> {
        if self.task_repository.find_by_id(task_id).await?.is_none() {
            return Err(TaskRepositoryError::NotFound(task_id).into());
        }

        self.task_result_repository
            .find_by_task_id(task_id)
            .await
            .map_err(Into::into)
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
