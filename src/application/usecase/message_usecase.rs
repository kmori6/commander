use uuid::Uuid;

use crate::application::error::message_usecase_error::MessageUsecaseError;
use crate::domain::error::message_repository_error::MessageRepositoryError;
use crate::domain::model::message::{Message, MessageContent, Role};
use crate::domain::model::session::SessionStatus;
use crate::domain::repository::message_repository::MessageRepository;
use crate::domain::repository::session_repository::SessionRepository;

pub struct MessageUsecase<M, S> {
    message_repository: M,
    session_repository: S,
}

impl<M, S> MessageUsecase<M, S>
where
    M: MessageRepository,
    S: SessionRepository,
{
    pub fn new(message_repository: M, session_repository: S) -> Self {
        Self {
            message_repository,
            session_repository,
        }
    }

    pub async fn save_user_text(
        &self,
        session_id: Uuid,
        text: String,
    ) -> Result<Message, MessageUsecaseError> {
        self.ensure_active_session(session_id).await?;

        self.message_repository
            .save(
                session_id,
                Role::User,
                vec![MessageContent::input_text(text)],
            )
            .await
            .map_err(Into::into)
    }

    pub async fn save(
        &self,
        session_id: Uuid,
        role: Role,
        contents: Vec<MessageContent>,
    ) -> Result<Message, MessageUsecaseError> {
        self.ensure_active_session(session_id).await?;

        self.message_repository
            .save(session_id, role, contents)
            .await
            .map_err(Into::into)
    }

    pub async fn list_for_session(
        &self,
        session_id: Uuid,
    ) -> Result<Vec<Message>, MessageUsecaseError> {
        self.ensure_existing_session(session_id).await?;

        self.message_repository
            .list_for_session(session_id)
            .await
            .map_err(Into::into)
    }

    async fn ensure_existing_session(&self, session_id: Uuid) -> Result<(), MessageUsecaseError> {
        let session = self.session_repository.find_by_id(session_id).await?;

        if session.is_none() {
            return Err(MessageRepositoryError::SessionNotFound(session_id).into());
        }

        Ok(())
    }

    async fn ensure_active_session(&self, session_id: Uuid) -> Result<(), MessageUsecaseError> {
        let session = self.session_repository.find_by_id(session_id).await?;

        match session {
            Some(session) if session.status == SessionStatus::Active => Ok(()),
            Some(_) => Err(MessageRepositoryError::InvalidMessage(format!(
                "session is closed: {session_id}"
            ))
            .into()),
            None => Err(MessageRepositoryError::SessionNotFound(session_id).into()),
        }
    }
}
