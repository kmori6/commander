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
use crate::domain::model::task::Task;
use crate::presentation::state::app_state::AppState;

fn task_json(task: Task) -> serde_json::Value {
    json!({
        "id": task.id.to_string(),
        "request": task.request,
        "status": task.status.as_str(),
        "session_id": task.session_id.to_string(),
        "source_message_id": task.source_message_id.map(|id| id.to_string()),
        "parent_task_id": task.parent_task_id.map(|id| id.to_string()),
        "created_at": task.created_at.to_rfc3339(),
        "updated_at": task.updated_at.to_rfc3339(),
        "started_at": task.started_at.map(|dt| dt.to_rfc3339()),
        "finished_at": task.finished_at.map(|dt| dt.to_rfc3339()),
    })
}

fn schedule_execution_json(execution: ScheduleExecution) -> serde_json::Value {
    json!({
        "id": execution.id.to_string(),
        "schedule_id": execution.schedule_id.to_string(),
        "task_id": execution.task_id.to_string(),
        "scheduled_at": execution.scheduled_at.to_rfc3339(),
        "created_at": execution.created_at.to_rfc3339(),
    })
}

pub async fn run_schedule_handler(
    State(state): State<AppState>,
    Path(schedule_id): Path<Uuid>,
) -> Response {
    match state.schedule_usecase.run_now(schedule_id).await {
        Ok(run_task) => {
            let task_id = run_task.task.id;
            let agent_runtime = state.agent_runtime.clone();

            tokio::spawn(async move {
                if let Err(err) = agent_runtime.run(task_id, None).await {
                    log::warn!("failed to run schedule task {task_id}: {err}");
                }
            });

            (
                StatusCode::ACCEPTED,
                Json(json!({
                    "run": schedule_execution_json(run_task.execution),
                    "task": task_json(run_task.task),
                })),
            )
                .into_response()
        }
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
                    "code": "failed_to_run_schedule",
                    "message": err.to_string(),
                }
            })),
        )
            .into_response(),
    }
}
