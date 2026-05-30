use axum::{
    Json,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Deserialize;

use crate::application::error::schedule_usecase_error::ScheduleUsecaseError;
use crate::domain::error::schedule_repository_error::ScheduleRepositoryError;
use crate::presentation::dto::error::ErrorResponse;
use crate::presentation::dto::schedule::ScheduleResponse;
use crate::presentation::state::app_state::AppState;

#[derive(Debug, Deserialize)]
pub struct CreateScheduleRequest {
    pub title: String,
    pub request: String,
    pub cron: String,
    pub timezone: Option<String>,
    pub enabled: Option<bool>,
}

pub async fn create_schedule_handler(
    State(state): State<AppState>,
    Json(request): Json<CreateScheduleRequest>,
) -> Response {
    match state
        .schedule_usecase
        .create(
            request.title,
            request.request,
            request.cron,
            request.timezone.unwrap_or_else(|| "UTC".to_string()),
            request.enabled.unwrap_or(true),
        )
        .await
    {
        Ok(schedule) => {
            (StatusCode::CREATED, Json(ScheduleResponse::from(schedule))).into_response()
        }
        Err(ScheduleUsecaseError::ScheduleRepository(
            ScheduleRepositoryError::InvalidSchedule(message),
        )) => (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse::new("invalid_schedule", message)),
        )
            .into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse::new(
                "failed_to_create_schedule",
                err.to_string(),
            )),
        )
            .into_response(),
    }
}
