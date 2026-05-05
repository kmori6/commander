use axum::{
    Json,
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
};
use serde::Deserialize;
use serde_json::json;
use std::collections::HashMap;

use crate::domain::model::chat_session::ChatSession;
use crate::domain::repository::chat_message_repository::ChatMessageRepository;
use crate::domain::repository::chat_session_repository::ChatSessionRepository;
use crate::presentation::state::app_state::AppState;

const DEFAULT_LIMIT: usize = 20;
const MAX_LIMIT: usize = 100;
const UNTITLED_SESSION_TITLE: &str = "Untitled session";

#[derive(Debug, Deserialize)]
pub struct ListSessionQuery {
    pub limit: Option<usize>,
}

pub async fn list_session_handler(
    State(state): State<AppState>,
    Query(query): Query<ListSessionQuery>,
) -> impl IntoResponse {
    let limit = query.limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT);

    let sessions = match state.chat_session_repository.list_recent(limit).await {
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

    let session_ids = sessions
        .iter()
        .map(|session| session.id)
        .collect::<Vec<_>>();

    let message_summaries = match state
        .chat_message_repository
        .summarize_by_session_ids(&session_ids)
        .await
    {
        Ok(summaries) => summaries,
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

    let message_summaries = message_summaries
        .into_iter()
        .map(|summary| (summary.session_id, summary))
        .collect::<HashMap<_, _>>();

    let sessions = sessions
        .into_iter()
        .map(|session| {
            let message_summary = message_summaries.get(&session.id);
            let title = session
                .title
                .as_deref()
                .filter(|title| !title.trim().is_empty())
                .map(ToOwned::to_owned)
                .unwrap_or_else(|| {
                    message_summary
                        .and_then(|summary| summary.first_user_message.as_deref())
                        .and_then(ChatSession::title_from_first_user_message)
                        .unwrap_or_else(|| UNTITLED_SESSION_TITLE.to_string())
                });

            json!({
                "id": session.id.to_string(),
                "title": title,
                "status": session.status.as_str(),
                "created_at": session.created_at.to_rfc3339(),
                "updated_at": session.updated_at.to_rfc3339(),
                "message_count": message_summary
                    .map(|summary| summary.message_count)
                    .unwrap_or(0),
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
