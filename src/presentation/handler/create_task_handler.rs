use axum::{Json, extract::State, http::StatusCode, response::IntoResponse};
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use crate::domain::model::task::Task;
use crate::presentation::state::app_state::AppState;

#[derive(Debug, Deserialize)]
pub struct CreateTaskRequest {
    pub request: String,
    pub parent_task_id: Option<Uuid>,
}

fn task_json(task: Task) -> serde_json::Value {
    json!({
        "id": task.id.to_string(),
        "request": task.request,
        "status": task.status.as_str(),
        "session_id": task.session_id.map(|id| id.to_string()),
        "source_kind": task.source_kind.as_str(),
        "source_message_id": task.source_message_id.map(|id| id.to_string()),
        "source_schedule_id": task.source_schedule_id.map(|id| id.to_string()),
        "parent_task_id": task.parent_task_id.map(|id| id.to_string()),
        "scheduled_at": task.scheduled_at.map(|dt| dt.to_rfc3339()),
        "output": task.output,
        "error": task.error,
        "created_at": task.created_at.to_rfc3339(),
        "updated_at": task.updated_at.to_rfc3339(),
        "started_at": task.started_at.map(|dt| dt.to_rfc3339()),
        "finished_at": task.finished_at.map(|dt| dt.to_rfc3339()),
    })
}

pub async fn create_task_handler(
    State(state): State<AppState>,
    Json(request): Json<CreateTaskRequest>,
) -> impl IntoResponse {
    match state
        .task_usecase
        .create(request.request, request.parent_task_id)
        .await
    {
        Ok(task) => (StatusCode::CREATED, Json(task_json(task))),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "error": {
                    "code": "failed_to_create_task",
                    "message": err.to_string(),
                }
            })),
        ),
    }
}
