use axum::{Json, extract::State, http::StatusCode, response::IntoResponse};
use serde::Deserialize;
use serde_json::json;

use crate::application::error::schedule_usecase_error::ScheduleUsecaseError;
use crate::domain::error::schedule_repository_error::ScheduleRepositoryError;
use crate::domain::model::schedule::Schedule;
use crate::presentation::state::app_state::AppState;

#[derive(Debug, Deserialize)]
pub struct CreateScheduleRequest {
    pub title: String,
    pub request: String,
    pub cron: String,
    pub enabled: Option<bool>,
}

fn schedule_json(schedule: Schedule) -> serde_json::Value {
    json!({
        "id": schedule.id.to_string(),
        "title": schedule.title,
        "request": schedule.request,
        "cron": schedule.cron,
        "enabled": schedule.enabled,
        "created_at": schedule.created_at.to_rfc3339(),
        "updated_at": schedule.updated_at.to_rfc3339(),
    })
}

pub async fn create_schedule_handler(
    State(state): State<AppState>,
    Json(request): Json<CreateScheduleRequest>,
) -> impl IntoResponse {
    match state
        .schedule_usecase
        .create(
            request.title,
            request.request,
            request.cron,
            request.enabled.unwrap_or(true),
        )
        .await
    {
        Ok(schedule) => (StatusCode::CREATED, Json(schedule_json(schedule))),
        Err(ScheduleUsecaseError::ScheduleRepository(
            ScheduleRepositoryError::InvalidSchedule(message),
        )) => (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": {
                    "code": "invalid_schedule",
                    "message": message,
                }
            })),
        ),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "error": {
                    "code": "failed_to_create_schedule",
                    "message": err.to_string(),
                }
            })),
        ),
    }
}
