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
use crate::domain::model::message::TaskUsage;
use crate::presentation::state::app_state::AppState;

fn task_usage_json(usage: TaskUsage) -> serde_json::Value {
    json!({
        "task_id": usage.task_id.to_string(),
        "input_tokens": usage.input_tokens,
        "output_tokens": usage.output_tokens,
        "cache_read_tokens": usage.cache_read_tokens,
        "cache_write_tokens": usage.cache_write_tokens,
        "total_tokens": usage.total_tokens(),
    })
}

pub async fn get_task_usage_handler(
    State(state): State<AppState>,
    Path(task_id): Path<Uuid>,
) -> Response {
    match state.task_usecase.find_usage(task_id).await {
        Ok(usage) => (StatusCode::OK, Json(task_usage_json(usage))).into_response(),
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
                    "code": "failed_to_get_task_usage",
                    "message": err.to_string(),
                }
            })),
        )
            .into_response(),
    }
}
