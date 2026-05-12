use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use crate::application::error::schedule_usecase_error::ScheduleUsecaseError;
use crate::domain::error::schedule_repository_error::ScheduleRepositoryError;
use crate::domain::model::schedule::Schedule;
use crate::domain::repository::schedule_repository::UpdateSchedule;
use crate::presentation::state::app_state::AppState;

#[derive(Debug, Deserialize)]
pub struct UpdateScheduleRequest {
    pub title: Option<String>,
    pub request: Option<String>,
    pub cron: Option<String>,
    pub timezone: Option<String>,
    pub enabled: Option<bool>,
}

fn schedule_json(schedule: Schedule) -> serde_json::Value {
    json!({
        "id": schedule.id.to_string(),
        "title": schedule.title,
        "request": schedule.request,
        "cron": schedule.cron,
        "timezone": schedule.timezone,
        "enabled": schedule.enabled,
        "created_at": schedule.created_at.to_rfc3339(),
        "updated_at": schedule.updated_at.to_rfc3339(),
    })
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
        Ok(schedule) => (StatusCode::OK, Json(schedule_json(schedule))).into_response(),
        Err(ScheduleUsecaseError::ScheduleRepository(ScheduleRepositoryError::NotFound(_))) => (
            StatusCode::NOT_FOUND,
            Json(json!({
                "error": {
                    "code": "schedule_not_found",
                    "message": format!("schedule not found: {schedule_id}"),
                }
            })),
        )
            .into_response(),
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
        )
            .into_response(),

        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "error": {
                    "code": "failed_to_update_schedule",
                    "message": err.to_string(),
                }
            })),
        )
            .into_response(),
    }
}
