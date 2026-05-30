use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use uuid::Uuid;

use crate::application::error::schedule_usecase_error::ScheduleUsecaseError;
use crate::domain::error::schedule_repository_error::ScheduleRepositoryError;
use crate::domain::repository::schedule_repository::UpdateSchedule;
use crate::presentation::dto::error::ErrorResponse;
use crate::presentation::dto::schedule::ScheduleResponse;
use crate::presentation::state::app_state::AppState;

#[derive(Debug, Deserialize)]
pub struct UpdateScheduleRequest {
    pub title: Option<String>,
    pub request: Option<String>,
    pub cron: Option<String>,
    pub timezone: Option<String>,
    pub enabled: Option<bool>,
}

pub async fn update_schedule_handler(
    State(state): State<AppState>,
    Path(schedule_id): Path<Uuid>,
    Json(request): Json<UpdateScheduleRequest>,
) -> Response {
    match state
        .schedule_usecase
        .update(
            schedule_id,
            UpdateSchedule {
                title: request.title,
                request: request.request,
                cron: request.cron,
                timezone: request.timezone,
                enabled: request.enabled,
            },
        )
        .await
    {
        Ok(schedule) => (StatusCode::OK, Json(ScheduleResponse::from(schedule))).into_response(),
        Err(ScheduleUsecaseError::ScheduleRepository(ScheduleRepositoryError::NotFound(_))) => (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse::new(
                "schedule_not_found",
                format!("schedule not found: {schedule_id}"),
            )),
        )
            .into_response(),
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
                "failed_to_update_schedule",
                err.to_string(),
            )),
        )
            .into_response(),
    }
}
