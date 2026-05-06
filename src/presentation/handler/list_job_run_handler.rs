use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde_json::{Value, json};
use uuid::Uuid;

use crate::application::error::job_usecase_error::JobUsecaseError;
use crate::domain::model::job_run::JobRun;
use crate::presentation::state::app_state::AppState;

pub async fn list_job_run_handler(
    State(state): State<AppState>,
    Path(job_id): Path<Uuid>,
) -> Response {
    match state.job_usecase.list_runs(job_id).await {
        Ok(runs) => {
            let runs = runs.into_iter().map(job_run_json).collect::<Vec<_>>();

            (
                StatusCode::OK,
                Json(json!({
                    "runs": runs,
                })),
            )
                .into_response()
        }
        Err(JobUsecaseError::JobNotFound(id)) => (
            StatusCode::NOT_FOUND,
            Json(json!({
                "error": {
                    "code": "job_not_found",
                    "message": format!("job not found: {id}"),
                }
            })),
        )
            .into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "error": {
                    "code": "failed_to_list_job_runs",
                    "message": err.to_string(),
                }
            })),
        )
            .into_response(),
    }
}

fn job_run_json(run: JobRun) -> Value {
    json!({
        "id": run.id.to_string(),
        "job_id": run.job_id.to_string(),
        "attempt": run.attempt,
        "status": run.status.as_str(),
        "started_at": run.started_at.to_rfc3339(),
        "finished_at": run.finished_at.map(|time| time.to_rfc3339()),
        "error_message": run.error_message,
    })
}
