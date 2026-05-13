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
use crate::domain::model::schedule_execution::ScheduleExecution;
use crate::presentation::state::app_state::AppState;

fn schedule_execution_json(execution: ScheduleExecution) -> serde_json::Value {
    json!({
        "id": execution.id.to_string(),
        "schedule_id": execution.schedule_id.to_string(),
        "task_id": execution.task_id.to_string(),
        "scheduled_at": execution.scheduled_at.to_rfc3339(),
        "created_at": execution.created_at.to_rfc3339(),
    })
}

pub async fn list_schedule_run_handler(
    State(state): State<AppState>,
    Path(schedule_id): Path<Uuid>,
) -> Response {
    match state.schedule_usecase.list_executions(schedule_id).await {
        Ok(runs) => (
            StatusCode::OK,
            Json(json!({
                "runs": runs.into_iter().map(schedule_execution_json).collect::<Vec<_>>(),
            })),
        )
            .into_response(),
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
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "error": {
                    "code": "failed_to_list_schedule_runs",
                    "message": err.to_string(),
                }
            })),
        )
            .into_response(),
    }
}
