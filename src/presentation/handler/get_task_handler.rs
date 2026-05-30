use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use uuid::Uuid;

use crate::presentation::dto::error::ErrorResponse;
use crate::presentation::dto::task::TaskResponse;
use crate::presentation::state::app_state::AppState;

pub async fn get_task_handler(
    State(state): State<AppState>,
    Path(task_id): Path<Uuid>,
) -> Response {
    match state.task_usecase.find(task_id).await {
        Ok(Some(task)) => (StatusCode::OK, Json(TaskResponse::from(task))).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse::new(
                "task_not_found",
                format!("task not found: {task_id}"),
            )),
        )
            .into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse::new("failed_to_get_task", err.to_string())),
        )
            .into_response(),
    }
}
