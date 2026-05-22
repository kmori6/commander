use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::broadcast;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Event {
    pub id: Uuid,
    pub task_id: Uuid,
    pub event_type: String,
    pub payload: Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone)]
pub struct EventService {
    tx: broadcast::Sender<Event>,
}

impl Default for EventService {
    fn default() -> Self {
        Self::new()
    }
}

impl EventService {
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(1024);
        Self { tx }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Event> {
        self.tx.subscribe()
    }

    pub fn publish(&self, task_id: Uuid, event_type: impl Into<String>, payload: Value) {
        let event = Event {
            id: Uuid::new_v4(),
            task_id,
            event_type: event_type.into(),
            payload,
            created_at: Utc::now(),
        };

        let _ = self.tx.send(event);
    }
}
