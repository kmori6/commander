use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde_json::json;
use uuid::Uuid;

use crate::presentation::dto::error::ErrorResponse;
use crate::presentation::dto::message::MessageResponse;
use crate::presentation::state::app_state::AppState;

pub async fn list_message_handler(
    State(state): State<AppState>,
    Path(session_id): Path<Uuid>,
) -> Response {
    match state.message_usecase.list_for_session(session_id).await {
        Ok(messages) => (
            StatusCode::OK,
            Json(json!({
                "messages": messages
                    .into_iter()
                    .map(MessageResponse::from)
                    .collect::<Vec<_>>(),
            })),
        )
            .into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse::new(
                "failed_to_list_messages",
                err.to_string(),
            )),
        )
            .into_response(),
    }
}
