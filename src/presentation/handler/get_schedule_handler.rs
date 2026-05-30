use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use uuid::Uuid;

use crate::presentation::dto::error::ErrorResponse;
use crate::presentation::dto::schedule::ScheduleResponse;
use crate::presentation::state::app_state::AppState;

pub async fn get_schedule_handler(
    State(state): State<AppState>,
    Path(schedule_id): Path<Uuid>,
) -> Response {
    match state.schedule_usecase.find(schedule_id).await {
        Ok(Some(schedule)) => {
            (StatusCode::OK, Json(ScheduleResponse::from(schedule))).into_response()
        }
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse::new(
                "schedule_not_found",
                format!("schedule not found: {schedule_id}"),
            )),
        )
            .into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse::new(
                "failed_to_get_schedule",
                err.to_string(),
            )),
        )
            .into_response(),
    }
}
