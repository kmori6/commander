use uuid::Uuid;

use crate::application::error::task_usecase_error::TaskUsecaseError;
use crate::domain::error::message_repository_error::MessageRepositoryError;
use crate::domain::error::task_repository_error::TaskRepositoryError;
use crate::domain::model::message::{Message, TaskUsage};
use crate::domain::model::task::{Task, TaskSource, TaskStatus};
use crate::domain::repository::message_repository::MessageRepository;
use crate::domain::repository::task_repository::TaskRepository;

pub struct TaskUsecase<T, M> {
    task_repository: T,
    message_repository: M,
}

impl<T, M> TaskUsecase<T, M>
where
    T: TaskRepository,
    M: MessageRepository,
{
    pub fn new(task_repository: T, message_repository: M) -> Self {
        Self {
            task_repository,
            message_repository,
        }
    }

    pub async fn create(&self, request: String) -> Result<Task, TaskUsecaseError> {
        let request = request.trim().to_string();

        if request.is_empty() {
            return Err(TaskRepositoryError::InvalidTask(
                "task request must not be empty".to_string(),
            )
            .into());
        }

        let task = self.task_repository.create(TaskSource::Direct).await?;

        self.message_repository
            .save(Message::new_user_text(task.id, request).map_err(MessageRepositoryError::from)?)
            .await?;

        Ok(task)
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

        if task.can_cancel() {
            return self.task_repository.cancel(id).await.map_err(Into::into);
        }

        Ok(task)
    }

    pub async fn find_usage(&self, task_id: Uuid) -> Result<TaskUsage, TaskUsecaseError> {
        if self.task_repository.find_by_id(task_id).await?.is_none() {
            return Err(TaskRepositoryError::NotFound(task_id).into());
        }

        self.message_repository
            .task_usage(task_id)
            .await
            .map_err(Into::into)
    }
}
