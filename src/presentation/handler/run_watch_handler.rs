use axum::{
    Json,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use chrono::Utc;
use serde_json::json;

use crate::application::usecase::schedule_usecase::DueTaskOutcome;
use crate::presentation::dto::error::ErrorResponse;
use crate::presentation::dto::task::TaskResponse;
use crate::presentation::state::app_state::AppState;

pub async fn run_watch_handler(State(state): State<AppState>) -> Response {
    let Some(request) = state.instruction_service.build_watch_request() else {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse::new(
                "watch_request_missing",
                "WATCH.md is missing or empty",
            )),
        )
            .into_response();
    };

    match state
        .schedule_usecase
        .run_due_task(request, None, Utc::now())
        .await
    {
        Ok(DueTaskOutcome::Started(task)) | Ok(DueTaskOutcome::AlreadyRecorded(task)) => (
            StatusCode::ACCEPTED,
            Json(json!({
                "task": TaskResponse::from(task),
            })),
        )
            .into_response(),
        Ok(DueTaskOutcome::NoRequest) => (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse::new(
                "watch_request_empty",
                "WATCH.md is empty",
            )),
        )
            .into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse::new("failed_to_run_watch", err.to_string())),
        )
            .into_response(),
    }
}
