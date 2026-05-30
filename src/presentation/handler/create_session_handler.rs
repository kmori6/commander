use axum::{
    Json,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Deserialize;

use crate::presentation::dto::error::ErrorResponse;
use crate::presentation::dto::session::SessionResponse;
use crate::presentation::state::app_state::AppState;

#[derive(Debug, Deserialize)]
pub struct CreateSessionRequest {
    pub title: Option<String>,
}

pub async fn create_session_handler(
    State(state): State<AppState>,
    Json(request): Json<CreateSessionRequest>,
) -> Response {
    match state.session_usecase.create_chat(request.title).await {
        Ok(session) => (StatusCode::CREATED, Json(SessionResponse::from(session))).into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse::new(
                "failed_to_create_session",
                err.to_string(),
            )),
        )
            .into_response(),
    }
}
