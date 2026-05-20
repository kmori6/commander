use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde_json::json;
use uuid::Uuid;

use crate::domain::model::task::Task;
use crate::presentation::state::app_state::AppState;

fn task_json(task: Task) -> serde_json::Value {
    json!({
        "id": task.id.to_string(),
        "status": task.status.as_str(),
        "session_id": task.session_id.map(|id| id.to_string()),
        "source_schedule_id": task.source_schedule_id.map(|id| id.to_string()),
        "scheduled_at": task.scheduled_at.map(|dt| dt.to_rfc3339()),
        "error": task.error,
        "created_at": task.created_at.to_rfc3339(),
        "updated_at": task.updated_at.to_rfc3339(),
        "started_at": task.started_at.map(|dt| dt.to_rfc3339()),
        "finished_at": task.finished_at.map(|dt| dt.to_rfc3339()),
    })
}

pub async fn get_task_handler(
    State(state): State<AppState>,
    Path(task_id): Path<Uuid>,
) -> Response {
    match state.task_usecase.find(task_id).await {
        Ok(Some(task)) => (StatusCode::OK, Json(task_json(task))).into_response(),
        Ok(None) => (
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
                    "code": "failed_to_get_task",
                    "message": err.to_string(),
                }
            })),
        )
            .into_response(),
    }
}
