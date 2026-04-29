use axum::{Json, extract::State, http::StatusCode, response::IntoResponse};
use serde_json::json;

use crate::domain::repository::chat_session_repository::ChatSessionRepository;
use crate::infrastructure::persistence::postgres_chat_session_repository::PostgresChatSessionRepository;

pub async fn create_session_handler(
    State(chat_session_repository): State<PostgresChatSessionRepository>,
) -> impl IntoResponse {
    match chat_session_repository.create().await {
        Ok(session) => (
            StatusCode::CREATED,
            Json(json!({
                "id": session.id.to_string(),
                "created_at": session.created_at.to_rfc3339(),
                "updated_at": session.updated_at.to_rfc3339(),
            })),
        ),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "error": {
                    "code": "failed_to_create_session",
                    "message": err.to_string(),
                }
            })),
        ),
    }
}
