use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::application::error::session_usecase_error::SessionUsecaseError;
use crate::domain::error::session_repository_error::SessionRepositoryError;
use crate::domain::model::session::Session;
use crate::presentation::state::app_state::AppState;

#[derive(Debug, Deserialize)]
pub struct UpdateSessionRequest {
    pub title: Option<String>,
}

fn session_json(session: Session) -> Value {
    json!({
        "id": session.id.to_string(),
        "title": session.title,
        "created_at": session.created_at.to_rfc3339(),
        "updated_at": session.updated_at.to_rfc3339(),
    })
}

pub async fn update_session_handler(
    State(state): State<AppState>,
    Path(session_id): Path<Uuid>,
    Json(request): Json<UpdateSessionRequest>,
) -> Response {
    let result = state
        .session_usecase
        .update_title(session_id, request.title)
        .await;

    match result {
        Ok(session) => (StatusCode::OK, Json(session_json(session))).into_response(),
        Err(SessionUsecaseError::SessionRepository(SessionRepositoryError::NotFound(_))) => (
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
                    "code": "failed_to_update_session",
                    "message": err.to_string(),
                }
            })),
        )
            .into_response(),
    }
}
