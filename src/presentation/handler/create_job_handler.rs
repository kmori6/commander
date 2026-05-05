use crate::domain::model::job::{Job, JobKind};
use crate::presentation::state::app_state::AppState;
use axum::{
    Json,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use serde_json::{Value, json};
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct CreateJobRequest {
    pub kind: String,
    pub title: String,
    pub objective: String,
    pub session_id: Option<Uuid>,
    pub parent_job_id: Option<Uuid>,
}

pub async fn create_job_handler(
    State(state): State<AppState>,
    Json(request): Json<CreateJobRequest>,
) -> Response {
    let Some(kind) = JobKind::parse(&request.kind) else {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": {
                    "code": "invalid_job_kind",
                    "message": format!("invalid job kind: {}", request.kind),
                }
            })),
        )
            .into_response();
    };

    match state
        .job_usecase
        .create(
            kind,
            request.title,
            request.objective,
            request.session_id,
            request.parent_job_id,
        )
        .await
    {
        Ok(job) => (StatusCode::CREATED, Json(job_json(job))).into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "error": {
                    "code": "failed_to_create_job",
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
