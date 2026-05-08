use std::{convert::Infallible, time::Duration};

use axum::{
    extract::{Query, State},
    response::sse::{Event as SseEvent, KeepAlive, Sse},
};
use futures::Stream;
use serde::Deserialize;
use serde_json::json;
use tokio::sync::broadcast;
use uuid::Uuid;

use crate::domain::model::event::Event;
use crate::presentation::state::app_state::AppState;

#[derive(Debug, Deserialize)]
pub struct EventStreamQuery {
    pub task_id: Option<Uuid>,
}

fn event_json(event: Event) -> serde_json::Value {
    json!({
        "id": event.id.to_string(),
        "task_id": event.task_id.to_string(),
        "event_type": event.event_type,
        "payload": event.payload,
        "created_at": event.created_at.to_rfc3339(),
    })
}

pub async fn get_event_handler(
    State(state): State<AppState>,
    Query(query): Query<EventStreamQuery>,
) -> Sse<impl Stream<Item = Result<SseEvent, Infallible>>> {
    let rx = state.event_service.subscribe();
    let task_id = query.task_id;

    let stream = futures::stream::unfold(rx, move |mut rx| async move {
        loop {
            match rx.recv().await {
                Ok(event) => {
                    if task_id.is_some_and(|id| id != event.task_id) {
                        continue;
                    }

                    let event_type = event.event_type.clone();
                    let event_id = event.id.to_string();
                    let data = serde_json::to_string(&event_json(event))
                        .unwrap_or_else(|_| "{}".to_string());

                    let sse = SseEvent::default()
                        .event(event_type)
                        .id(event_id)
                        .data(data);

                    return Some((Ok(sse), rx));
                }
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
                Err(broadcast::error::RecvError::Closed) => return None,
            }
        }
    });

    Sse::new(stream).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("keep-alive"),
    )
}
