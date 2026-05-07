use uuid::Uuid;

use crate::application::error::session_usecase_error::SessionUsecaseError;
use crate::domain::model::session::{Session, SessionKind, SessionStatus};
use crate::domain::repository::session_repository::SessionRepository;

pub struct SessionUsecase<R> {
    session_repository: R,
}

impl<R> SessionUsecase<R>
where
    R: SessionRepository,
{
    pub fn new(session_repository: R) -> Self {
        Self { session_repository }
    }

    pub async fn create_chat(&self, title: Option<String>) -> Result<Session, SessionUsecaseError> {
        self.session_repository
            .create(SessionKind::Chat, title)
            .await
            .map_err(Into::into)
    }

    pub async fn create(
        &self,
        kind: SessionKind,
        title: Option<String>,
    ) -> Result<Session, SessionUsecaseError> {
        self.session_repository
            .create(kind, title)
            .await
            .map_err(Into::into)
    }

    pub async fn find(&self, id: Uuid) -> Result<Option<Session>, SessionUsecaseError> {
        self.session_repository
            .find_by_id(id)
            .await
            .map_err(Into::into)
    }

    pub async fn list(
        &self,
        kind: Option<SessionKind>,
        limit: usize,
    ) -> Result<Vec<Session>, SessionUsecaseError> {
        self.session_repository
            .list_recent(kind, limit)
            .await
            .map_err(Into::into)
    }

    pub async fn update_title(
        &self,
        id: Uuid,
        title: Option<String>,
    ) -> Result<Session, SessionUsecaseError> {
        self.session_repository
            .update_title(id, title)
            .await
            .map_err(Into::into)
    }

    pub async fn close(&self, id: Uuid) -> Result<Session, SessionUsecaseError> {
        self.session_repository
            .update_status(id, SessionStatus::Closed)
            .await
            .map_err(Into::into)
    }
}
