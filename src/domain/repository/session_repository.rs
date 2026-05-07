use async_trait::async_trait;
use uuid::Uuid;

use crate::domain::error::session_repository_error::SessionRepositoryError;
use crate::domain::model::session::{Session, SessionKind, SessionStatus};

#[async_trait]
pub trait SessionRepository: Send + Sync {
    async fn create(
        &self,
        kind: SessionKind,
        title: Option<String>,
    ) -> Result<Session, SessionRepositoryError>;

    async fn find_by_id(&self, id: Uuid) -> Result<Option<Session>, SessionRepositoryError>;

    async fn list_recent(
        &self,
        kind: Option<SessionKind>,
        limit: usize,
    ) -> Result<Vec<Session>, SessionRepositoryError>;

    async fn update_title(
        &self,
        id: Uuid,
        title: Option<String>,
    ) -> Result<Session, SessionRepositoryError>;

    async fn update_status(
        &self,
        id: Uuid,
        status: SessionStatus,
    ) -> Result<Session, SessionRepositoryError>;
}
