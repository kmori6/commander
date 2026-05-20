use crate::domain::model::task::{Task, TaskStatus};
use crate::presentation::state::app_state::AppState;
use axum::{
    Json,
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
};
use serde::Deserialize;
use serde_json::json;

const DEFAULT_LIMIT: usize = 20;
const MAX_LIMIT: usize = 100;

#[derive(Debug, Deserialize)]
pub struct ListTaskQuery {
    pub status: Option<TaskStatus>,
    pub limit: Option<usize>,
}

fn task_json(task: Task) -> serde_json::Value {
    json!({
        "id": task.id.to_string(),
        "request": task.request,
        "status": task.status.as_str(),
        "session_id": task.session_id.map(|id| id.to_string()),
        "source_schedule_id": task.source_schedule_id.map(|id| id.to_string()),
        "scheduled_at": task.scheduled_at.map(|dt| dt.to_rfc3339()),
        "output": task.output,
        "error": task.error,
        "created_at": task.created_at.to_rfc3339(),
        "updated_at": task.updated_at.to_rfc3339(),
        "started_at": task.started_at.map(|dt| dt.to_rfc3339()),
        "finished_at": task.finished_at.map(|dt| dt.to_rfc3339()),
    })
}

pub async fn list_task_handler(
    State(state): State<AppState>,
    Query(query): Query<ListTaskQuery>,
) -> impl IntoResponse {
    let limit = query.limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT);

    match state.task_usecase.list(query.status, limit).await {
        Ok(tasks) => (
            StatusCode::OK,
            Json(json!({
                "tasks": tasks.into_iter().map(task_json).collect::<Vec<_>>(),
            })),
        ),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "error": {
                    "code": "failed_to_list_tasks",
                    "message": err.to_string(),
                }
            })),
        ),
    }
}
