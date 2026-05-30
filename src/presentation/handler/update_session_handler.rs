use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use uuid::Uuid;

use crate::application::error::session_usecase_error::SessionUsecaseError;
use crate::domain::error::session_repository_error::SessionRepositoryError;
use crate::presentation::dto::error::ErrorResponse;
use crate::presentation::dto::session::SessionResponse;
use crate::presentation::state::app_state::AppState;

#[derive(Debug, Deserialize)]
pub struct UpdateSessionRequest {
    pub title: Option<String>,
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
        Ok(session) => (StatusCode::OK, Json(SessionResponse::from(session))).into_response(),
        Err(SessionUsecaseError::SessionRepository(SessionRepositoryError::NotFound(_))) => (
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
                "failed_to_update_session",
                err.to_string(),
            )),
        )
            .into_response(),
    }
}
