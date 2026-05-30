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
use crate::presentation::state::app_state::AppState;

pub async fn list_schedule_run_handler(
    State(state): State<AppState>,
    Path(schedule_id): Path<Uuid>,
) -> Response {
    match state.schedule_usecase.list_runs(schedule_id).await {
        Ok(runs) => (
            StatusCode::OK,
            Json(json!({
                "runs": runs
                    .into_iter()
                    .map(|task| ScheduleRunResponse::new(schedule_id, &task))
                    .collect::<Vec<_>>(),
            })),
        )
            .into_response(),
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
                "failed_to_list_schedule_runs",
                err.to_string(),
            )),
        )
            .into_response(),
    }
}
