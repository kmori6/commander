use uuid::Uuid;

use crate::application::error::task_usecase_error::TaskUsecaseError;
use crate::domain::error::task_repository_error::TaskRepositoryError;
use crate::domain::model::session::SessionKind;
use crate::domain::model::task::{Task, TaskStatus};
use crate::domain::repository::session_repository::SessionRepository;
use crate::domain::repository::task_repository::{CreateTask, TaskRepository};

pub struct TaskUsecase<T, S> {
    task_repository: T,
    session_repository: S,
}

impl<T, S> TaskUsecase<T, S>
where
    T: TaskRepository,
    S: SessionRepository,
{
    pub fn new(task_repository: T, session_repository: S) -> Self {
        Self {
            task_repository,
            session_repository,
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
}
