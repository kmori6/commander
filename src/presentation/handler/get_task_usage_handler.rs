use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use uuid::Uuid;

use crate::application::error::task_usecase_error::TaskUsecaseError;
use crate::domain::error::task_repository_error::TaskRepositoryError;
use crate::presentation::dto::error::ErrorResponse;
use crate::presentation::dto::message::TaskUsageResponse;
use crate::presentation::state::app_state::AppState;

pub async fn get_task_usage_handler(
    State(state): State<AppState>,
    Path(task_id): Path<Uuid>,
) -> Response {
    match state.task_usecase.find_usage(task_id).await {
        Ok(usage) => (StatusCode::OK, Json(TaskUsageResponse::from(usage))).into_response(),
        Err(TaskUsecaseError::TaskRepository(TaskRepositoryError::NotFound(_))) => (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse::new(
                "task_not_found",
                format!("task not found: {task_id}"),
            )),
        )
            .into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse::new(
                "failed_to_get_task_usage",
                err.to_string(),
            )),
        )
            .into_response(),
    }
}
