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
use crate::domain::model::task_result::TaskResult;
use crate::presentation::state::app_state::AppState;

fn task_result_json(result: TaskResult) -> serde_json::Value {
    json!({
        "id": result.id.to_string(),
        "task_id": result.task_id.to_string(),
        "status": result.status.as_str(),
        "output": result.output,
        "created_at": result.created_at.to_rfc3339(),
    })
}

pub async fn get_task_result_handler(
    State(state): State<AppState>,
    Path(task_id): Path<Uuid>,
) -> Response {
    match state.task_usecase.find_result(task_id).await {
        Ok(Some(result)) => (StatusCode::OK, Json(task_result_json(result))).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(json!({
                "error": {
                    "code": "task_result_not_found",
                    "message": format!("task result not found: {task_id}"),
                }
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
                    "code": "failed_to_get_task_result",
                    "message": err.to_string(),
                }
            })),
        )
            .into_response(),
    }
}
