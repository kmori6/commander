use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde_json::json;
use uuid::Uuid;

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

pub async fn get_schedule_handler(
    State(state): State<AppState>,
    Path(schedule_id): Path<Uuid>,
) -> Response {
    match state.schedule_usecase.find(schedule_id).await {
        Ok(Some(schedule)) => (StatusCode::OK, Json(schedule_json(schedule))).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(json!({
                "error": {
                    "code": "schedule_not_found",
                    "message": format!("schedule not found: {schedule_id}"),
                }
            })),
        )
            .into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "error": {
                    "code": "failed_to_get_schedule",
                    "message": err.to_string(),
                }
            })),
        )
            .into_response(),
    }
}
