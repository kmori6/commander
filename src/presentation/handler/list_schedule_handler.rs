use axum::{Json, extract::State, http::StatusCode, response::IntoResponse};
use serde_json::json;

use crate::domain::model::schedule::Schedule;
use crate::presentation::state::app_state::AppState;

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

pub async fn list_schedule_handler(State(state): State<AppState>) -> impl IntoResponse {
    match state.schedule_usecase.list().await {
        Ok(schedules) => (
            StatusCode::OK,
            Json(json!({
                "schedules": schedules.into_iter().map(schedule_json).collect::<Vec<_>>(),
            })),
        ),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "error": {
                    "code": "failed_to_list_schedules",
                    "message": err.to_string(),
                }
            })),
        ),
    }
}
