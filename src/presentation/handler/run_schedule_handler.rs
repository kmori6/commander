use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde_json::json;
use uuid::Uuid;

use crate::application::error::schedule_usecase_error::ScheduleUsecaseError;
use crate::domain::error::schedule_repository_error::ScheduleRepositoryError;
use crate::presentation::dto::error::ErrorResponse;
use crate::presentation::dto::schedule::ScheduleRunResponse;
use crate::presentation::dto::task::TaskResponse;
use crate::presentation::state::app_state::AppState;

pub async fn run_schedule_handler(
    State(state): State<AppState>,
    Path(schedule_id): Path<Uuid>,
) -> Response {
    match state.schedule_usecase.run_now(schedule_id).await {
        Ok(task) => {
            let run = ScheduleRunResponse::new(schedule_id, &task);

            (
                StatusCode::ACCEPTED,
                Json(json!({
                    "run": run,
                    "task": TaskResponse::from(task),
                })),
            )
                .into_response()
        }
        Err(ScheduleUsecaseError::ScheduleRepository(ScheduleRepositoryError::NotFound(_))) => (
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
                "failed_to_run_schedule",
                err.to_string(),
            )),
        )
            .into_response(),
    }
}
