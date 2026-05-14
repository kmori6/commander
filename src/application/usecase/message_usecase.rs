use uuid::Uuid;

use crate::application::error::message_usecase_error::MessageUsecaseError;
use crate::domain::model::message::{Message, MessageContent, Role};
use crate::domain::model::task::{Task, TaskSourceKind};
use crate::domain::repository::message_repository::MessageRepository;
use crate::domain::repository::session_repository::SessionRepository;
use crate::domain::repository::task_repository::{CreateTask, TaskRepository};
pub struct MessageTask {
    pub message: Message,
    pub task: Task,
}

pub struct MessageUsecase<M, S, T> {
    message_repository: M,
    session_repository: S,
    task_repository: T,
}

impl<M, S, T> MessageUsecase<M, S, T>
where
    M: MessageRepository,
    S: SessionRepository,
    T: TaskRepository,
{
    pub fn new(message_repository: M, session_repository: S, task_repository: T) -> Self {
        Self {
            message_repository,
            session_repository,
            task_repository,
        }
    }

    pub async fn save_user_text(
        &self,
        session_id: Uuid,
        text: String,
    ) -> Result<MessageTask, MessageUsecaseError> {
        self.ensure_existing_session(session_id).await?;

        let task = self
            .task_repository
            .create(CreateTask {
                request: text.clone(),
                session_id: Some(session_id),
                source_kind: TaskSourceKind::Chat,
                source_message_id: None,
                source_schedule_id: None,
                parent_task_id: None,
                scheduled_at: None,
            })
            .await?;

        let message = self
            .message_repository
            .save(task.id, Role::User, vec![MessageContent::input_text(text)])
            .await?;

        Ok(MessageTask { message, task })
    }

    pub async fn save_for_task(
        &self,
        task_id: Uuid,
        role: Role,
        contents: Vec<MessageContent>,
    ) -> Result<Message, MessageUsecaseError> {
        self.message_repository
            .save(task_id, role, contents)
            .await
            .map_err(Into::into)
    }

    pub async fn list_for_session(
        &self,
        session_id: Uuid,
    ) -> Result<Vec<Message>, MessageUsecaseError> {
        self.ensure_existing_session(session_id).await?;

        let tasks = self.task_repository.list_by_session_id(session_id).await?;

        let mut messages = Vec::new();
        for task in tasks {
            messages.extend(self.message_repository.list_for_task(task.id).await?);
        }

        messages.sort_by_key(|message| (message.created_at, message.id));
        Ok(messages)
    }

    async fn ensure_existing_session(&self, session_id: Uuid) -> Result<(), MessageUsecaseError> {
        let session = self.session_repository.find_by_id(session_id).await?;

        if session.is_none() {
            return Err(MessageUsecaseError::SessionNotFound(session_id));
        }

        Ok(())
    }
}
