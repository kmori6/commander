use crate::presentation::dto::error::ErrorResponse;
use crate::presentation::dto::task::TaskResponse;
use crate::presentation::state::app_state::AppState;
use axum::{
    Json,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct CreateTaskRequest {
    pub request: String,
}

pub async fn create_task_handler(
    State(state): State<AppState>,
    Json(request): Json<CreateTaskRequest>,
) -> Response {
    match state.task_usecase.create(request.request).await {
        Ok(task) => (StatusCode::ACCEPTED, Json(TaskResponse::from(task))).into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse::new("failed_to_create_task", err.to_string())),
        )
            .into_response(),
    }
}
