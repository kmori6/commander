use axum::{Json, extract::State, http::StatusCode, response::IntoResponse};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::domain::model::session::Session;
use crate::presentation::state::app_state::AppState;

#[derive(Debug, Deserialize)]
pub struct CreateSessionRequest {
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

pub async fn create_session_handler(
    State(state): State<AppState>,
    Json(request): Json<CreateSessionRequest>,
) -> impl IntoResponse {
    match state.session_usecase.create_chat(request.title).await {
        Ok(session) => (StatusCode::CREATED, Json(session_json(session))),
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
