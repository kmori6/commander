use crate::application::error::job_execution_usecase_error::JobExecutionUsecaseError;
use crate::domain::error::job_error::JobError;
use crate::domain::error::job_run_error::JobRunError;
use crate::domain::model::app_event::AppEvent;
use crate::domain::model::job::Job;
use crate::domain::model::job_run::JobRun;
use crate::presentation::state::app_state::AppState;
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde_json::{Value, json};
use tokio::sync::mpsc;
use uuid::Uuid;

pub async fn create_job_run_handler(
    State(state): State<AppState>,
    Path(job_id): Path<Uuid>,
) -> Response {
    match state.job_execution_usecase.create_run(job_id).await {
        Ok(output) => {
            for event in output.events {
                state.event_service.publish(event);
            }

            let run_id = output.run.id;
            let usecase = state.job_execution_usecase.clone();
            let event_service = state.event_service.clone();

            tokio::spawn(async move {
                let (event_tx, mut event_rx) = mpsc::channel::<AppEvent>(32);

                let publisher_event_service = event_service.clone();
                let event_publisher = tokio::spawn(async move {
                    while let Some(event) = event_rx.recv().await {
                        publisher_event_service.publish(event);
                    }
                });

                match usecase.execute_run(job_id, run_id, event_tx).await {
                    Ok(output) => {
                        for event in output.events {
                            event_service.publish(event);
                        }
                    }
                    Err(err) => {
                        log::warn!("failed to execute job run {run_id}: {err}");
                    }
                }

                if let Err(err) = event_publisher.await {
                    log::warn!("failed to publish job run events for run {run_id}: {err}");
                }
            });

            (
                StatusCode::ACCEPTED,
                Json(json!({
                    "job": job_json(output.job),
                    "run": job_run_json(output.run),
                })),
            )
                .into_response()
        }
        Err(JobExecutionUsecaseError::JobNotFound(id)) => (
            StatusCode::NOT_FOUND,
            Json(json!({
                "error": {
                    "code": "job_not_found",
                    "message": format!("job not found: {id}"),
                }
            })),
        )
            .into_response(),
        Err(JobExecutionUsecaseError::Job(JobError::InvalidStatusTransition {
            job_id,
            status,
        })) => (
            StatusCode::CONFLICT,
            Json(json!({
                "error": {
                    "code": "invalid_job_status_transition",
                    "message": format!("cannot create run for job {job_id} from status {status}"),
                }
            })),
        )
            .into_response(),
        Err(JobExecutionUsecaseError::JobRun(JobRunError::InvalidStatusTransition {
            job_run_id,
            status,
        })) => (
            StatusCode::CONFLICT,
            Json(json!({
                "error": {
                    "code": "invalid_job_run_status_transition",
                    "message": format!("cannot complete job run {job_run_id} from status {status}"),
                }
            })),
        )
            .into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "error": {
                    "code": "failed_to_create_job_run",
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
