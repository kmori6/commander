use tokio::sync::broadcast;

use crate::domain::model::event::Event;

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

    pub fn publish(&self, event: Event) {
        let _ = self.tx.send(event);
    }
}
