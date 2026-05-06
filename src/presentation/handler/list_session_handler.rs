use axum::{
    Json,
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
};
use serde::Deserialize;
use serde_json::json;

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
) -> impl IntoResponse {
    let limit = query.limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT);

    let sessions = match state.chat_session_usecase.list(limit).await {
        Ok(sessions) => sessions,
        Err(err) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": {
                        "code": "failed_to_get_sessions",
                        "message": err.to_string(),
                    }
                })),
            );
        }
    };

    let sessions = sessions
        .into_iter()
        .map(|session| {
            json!({
                "id": session.id.to_string(),
                "title": session.title,
                "status": session.status.as_str(),
                "created_at": session.created_at.to_rfc3339(),
                "updated_at": session.updated_at.to_rfc3339(),
                "message_count": session.message_count,
            })
        })
        .collect::<Vec<_>>();

    (
        StatusCode::OK,
        Json(json!({
            "sessions": sessions,
        })),
    )
}
