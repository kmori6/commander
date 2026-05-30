use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use uuid::Uuid;

use crate::presentation::dto::error::ErrorResponse;
use crate::presentation::dto::session::SessionResponse;
use crate::presentation::state::app_state::AppState;

pub async fn get_session_handler(
    State(state): State<AppState>,
    Path(session_id): Path<Uuid>,
) -> Response {
    match state.session_usecase.find(session_id).await {
        Ok(Some(session)) => (StatusCode::OK, Json(SessionResponse::from(session))).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse::new(
                "session_not_found",
                format!("session not found: {session_id}"),
            )),
        )
            .into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse::new("failed_to_get_session", err.to_string())),
        )
            .into_response(),
    }
}
