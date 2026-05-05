use axum::{
    Json,
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::domain::model::job::Job;
use crate::presentation::state::app_state::AppState;

const DEFAULT_LIMIT: i64 = 20;
const MAX_LIMIT: i64 = 100;

#[derive(Debug, Deserialize)]
pub struct ListJobQuery {
    pub limit: Option<i64>,
}

pub async fn list_job_handler(
    State(state): State<AppState>,
    Query(query): Query<ListJobQuery>,
) -> Response {
    let limit = query.limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT);

    match state.job_usecase.list_recent(limit).await {
        Ok(jobs) => {
            let jobs = jobs.into_iter().map(job_json).collect::<Vec<_>>();

            (
                StatusCode::OK,
                Json(json!({
                    "jobs": jobs,
                })),
            )
                .into_response()
        }
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "error": {
                    "code": "failed_to_list_jobs",
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
        "session_id": job.session_id.map(|id| id.to_string()),
        "created_at": job.created_at.to_rfc3339(),
        "started_at": job.started_at.map(|time| time.to_rfc3339()),
        "finished_at": job.finished_at.map(|time| time.to_rfc3339()),
        "error_message": job.error_message,
    })
}
