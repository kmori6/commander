use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::application::error::message_usecase_error::MessageUsecaseError;
use crate::presentation::dto::error::ErrorResponse;
use crate::presentation::dto::message::MessageResponse;
use crate::presentation::state::app_state::AppState;

#[derive(Debug, Deserialize)]
pub struct CreateMessageRequest {
    pub text: String,

    #[serde(default)]
    pub input_images: Vec<Value>,

    #[serde(default)]
    pub input_files: Vec<Value>,
}

pub async fn create_message_handler(
    State(state): State<AppState>,
    Path(session_id): Path<Uuid>,
    Json(request): Json<CreateMessageRequest>,
) -> Response {
    if !request.input_images.is_empty() || !request.input_files.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse::new(
                "attachments_not_supported",
                "chat image/file attachments are not supported; place files in the workspace and reference their paths",
            )),
        )
            .into_response();
    }

    let text = request.text;

    match state.message_usecase.save_user_text(session_id, text).await {
        Ok(message_task) => {
            let task_id = message_task.task.id;

            (
                StatusCode::ACCEPTED,
                Json(json!({
                    "message": MessageResponse::from(message_task.message),
                    "task_id": task_id.to_string(),
                })),
            )
                .into_response()
        }
        Err(MessageUsecaseError::MessageRepository(_)) => (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse::new(
                "session_not_found",
                format!("session not found: {session_id}"),
            )),
        )
            .into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse::new(
                "failed_to_create_message",
                err.to_string(),
            )),
        )
            .into_response(),
    }
}
