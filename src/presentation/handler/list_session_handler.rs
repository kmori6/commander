use axum::{
    Json,
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use serde_json::json;

use crate::presentation::dto::error::ErrorResponse;
use crate::presentation::dto::session::SessionResponse;
use crate::presentation::state::app_state::AppState;

const DEFAULT_LIMIT: usize = 20;
const MAX_LIMIT: usize = 100;

#[derive(Debug, Deserialize)]
pub struct ListSessionQuery {
    pub limit: Option<usize>,
}

pub async fn list_session_handler(
    State(state): State<AppState>,
    Query(query): Query<ListSessionQuery>,
) -> Response {
    let limit = query.limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT);

    match state.session_usecase.list(limit).await {
        Ok(sessions) => (
            StatusCode::OK,
            Json(json!({
                "sessions": sessions
                    .into_iter()
                    .map(SessionResponse::from)
                    .collect::<Vec<_>>(),
            })),
        )
            .into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse::new(
                "failed_to_list_sessions",
                err.to_string(),
            )),
        )
            .into_response(),
    }
}
