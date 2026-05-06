use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde_json::json;
use uuid::Uuid;

use crate::application::error::job_run_usecase_error::JobRunUsecaseError;
use crate::presentation::handler::list_message_handler::chat_message_to_json;
use crate::presentation::state::app_state::AppState;

pub async fn list_job_run_message_handler(
    State(state): State<AppState>,
    Path((job_id, run_id)): Path<(Uuid, Uuid)>,
) -> Response {
    match state.job_run_usecase.messages(job_id, run_id).await {
        Ok(messages) => {
            let messages = messages
                .into_iter()
                .map(chat_message_to_json)
                .collect::<Vec<_>>();

            (
                StatusCode::OK,
                Json(json!({
                    "job_id": job_id.to_string(),
                    "run_id": run_id.to_string(),
                    "messages": messages,
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
        Err(JobRunUsecaseError::JobRunNotFound(id)) => (
            StatusCode::NOT_FOUND,
            Json(json!({
                "error": {
                    "code": "job_run_not_found",
                    "message": format!("job run not found: {id}"),
                }
            })),
        )
            .into_response(),
        Err(JobRunUsecaseError::JobRunDoesNotBelongToJob { job_id, run_id }) => (
            StatusCode::NOT_FOUND,
            Json(json!({
                "error": {
                    "code": "job_run_not_found",
                    "message": format!("job run {run_id} not found for job {job_id}"),
                }
            })),
        )
            .into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "error": {
                    "code": "failed_to_list_job_run_messages",
                    "message": err.to_string(),
                }
            })),
        )
            .into_response(),
    }
}
