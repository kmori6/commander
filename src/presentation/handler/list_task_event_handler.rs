use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde_json::json;
use uuid::Uuid;

use crate::application::error::task_usecase_error::TaskUsecaseError;
use crate::domain::error::task_repository_error::TaskRepositoryError;
use crate::domain::model::event::Event;
use crate::presentation::state::app_state::AppState;

fn event_json(event: Event) -> serde_json::Value {
    json!({
        "id": event.id.to_string(),
        "task_id": event.task_id.to_string(),
        "event_type": event.event_type,
        "payload": event.payload,
        "created_at": event.created_at.to_rfc3339(),
    })
}

pub async fn list_task_event_handler(
    State(state): State<AppState>,
    Path(task_id): Path<Uuid>,
) -> Response {
    match state.task_usecase.list_events(task_id).await {
        Ok(events) => (
            StatusCode::OK,
            Json(json!({
                "events": events.into_iter().map(event_json).collect::<Vec<_>>(),
            })),
        )
            .into_response(),
        Err(TaskUsecaseError::TaskRepository(TaskRepositoryError::NotFound(_))) => (
            StatusCode::NOT_FOUND,
            Json(json!({
                "error": {
                    "code": "task_not_found",
                    "message": format!("task not found: {task_id}"),
                }
            })),
        )
            .into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "error": {
                    "code": "failed_to_list_task_events",
                    "message": err.to_string(),
                }
            })),
        )
            .into_response(),
    }
}
