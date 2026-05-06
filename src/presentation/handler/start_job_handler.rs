use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde_json::{Value, json};
use uuid::Uuid;

use crate::application::error::job_run_usecase_error::JobRunUsecaseError;
use crate::domain::error::job_error::JobError;
use crate::domain::model::job::Job;
use crate::domain::model::job_run::JobRun;
use crate::presentation::state::app_state::AppState;

pub async fn start_job_handler(
    State(state): State<AppState>,
    Path(job_id): Path<Uuid>,
) -> Response {
    match state.job_run_usecase.start(job_id).await {
        Ok(output) => {
            for event in output.events {
                state.event_service.publish(event);
            }

            (
                StatusCode::OK,
                Json(json!({
                    "job": job_json(output.job),
                    "run": output.run.map(job_run_json),
                })),
            )
                .into_response()
        }
        Err(JobRunUsecaseError::JobNotFound(id)) => (
            StatusCode::NOT_FOUND,
            Json(json!({
                "error": {
                    "code": "job_not_found",
                    "message": format!("job not found: {id}"),
                }
            })),
        )
            .into_response(),
        Err(JobRunUsecaseError::Job(JobError::InvalidStatusTransition { job_id, status })) => (
            StatusCode::CONFLICT,
            Json(json!({
                "error": {
                    "code": "invalid_job_status_transition",
                    "message": format!("cannot start job {job_id} from status {status}"),
                }
            })),
        )
            .into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "error": {
                    "code": "failed_to_start_job",
                    "message": err.to_string(),
                }
            })),
        )
            .into_response(),
    }
}

fn job_json(job: Job) -> Value {
    json!({
        "id": job.id.to_string(),
        "kind": job.kind.as_str(),
        "status": job.status.as_str(),
        "title": job.title,
        "objective": job.objective,
        "session_id": job.session_id.map(|id| id.to_string()),
        "parent_job_id": job.parent_job_id.map(|id| id.to_string()),
        "created_at": job.created_at.to_rfc3339(),
        "started_at": job.started_at.map(|time| time.to_rfc3339()),
        "finished_at": job.finished_at.map(|time| time.to_rfc3339()),
        "error_message": job.error_message,
    })
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
