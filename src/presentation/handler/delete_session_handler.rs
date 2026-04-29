use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde_json::json;
use uuid::Uuid;

use crate::domain::error::chat_repository_error::ChatRepositoryError;
use crate::domain::repository::chat_session_repository::ChatSessionRepository;
use crate::infrastructure::persistence::postgres_chat_session_repository::PostgresChatSessionRepository;

pub async fn delete_session_handler(
    State(chat_session_repository): State<PostgresChatSessionRepository>,
    Path(session_id): Path<Uuid>,
) -> Response {
    match chat_session_repository.delete_by_id(session_id).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(ChatRepositoryError::SessionNotFound(_)) => (
            StatusCode::NOT_FOUND,
            Json(json!({
                "error": {
                    "code": "session_not_found",
                    "message": format!("session not found: {session_id}"),
                }
            })),
        )
            .into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "error": {
                    "code": "failed_to_delete_session",
                    "message": err.to_string(),
                }
            })),
        )
            .into_response(),
    }
}
