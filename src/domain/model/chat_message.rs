use crate::domain::model::message::Message;
use chrono::{DateTime, Utc};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub id: Uuid,
    pub session_id: Uuid,
    pub message: Message,
    pub created_at: DateTime<Utc>,
}
