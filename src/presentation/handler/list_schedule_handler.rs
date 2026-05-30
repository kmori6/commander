use axum::{
    Json,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde_json::json;

use crate::presentation::dto::error::ErrorResponse;
use crate::presentation::dto::schedule::ScheduleResponse;
use crate::presentation::state::app_state::AppState;

pub async fn list_schedule_handler(State(state): State<AppState>) -> Response {
    match state.schedule_usecase.list().await {
        Ok(schedules) => (
            StatusCode::OK,
            Json(json!({
                "schedules": schedules
                    .into_iter()
                    .map(ScheduleResponse::from)
                    .collect::<Vec<_>>(),
            })),
        )
            .into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse::new(
                "failed_to_list_schedules",
                err.to_string(),
            )),
        )
            .into_response(),
    }
}
