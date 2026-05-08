use async_trait::async_trait;
use serde_json::Value;
use uuid::Uuid;

use crate::domain::error::event_repository_error::EventRepositoryError;
use crate::domain::model::event::Event;

#[async_trait]
pub trait EventRepository: Send + Sync {
    async fn save(
        &self,
        task_id: Uuid,
        event_type: &str,
        payload: Value,
    ) -> Result<Event, EventRepositoryError>;

    async fn list_for_task(&self, task_id: Uuid) -> Result<Vec<Event>, EventRepositoryError>;
}
