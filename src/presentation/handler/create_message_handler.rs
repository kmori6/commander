use crate::domain::error::chat_repository_error::ChatRepositoryError;
use crate::domain::model::chat_session::ChatSessionStatus;
use crate::domain::model::message::{Message, MessageContent};
use crate::domain::model::role::Role;
use crate::domain::repository::chat_message_repository::ChatMessageRepository;
use crate::domain::repository::chat_session_repository::ChatSessionRepository;
use crate::presentation::state::app_state::AppState;
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::{
        IntoResponse, Response,
        sse::{Event, KeepAlive, Sse},
    },
};
use futures::stream;
use serde::Deserialize;
use serde_json::json;
use std::{convert::Infallible, time::Duration};
use tokio::sync::mpsc;
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct CreateMessageRequest {
    pub input: Vec<Message>,
}

pub async fn create_message_hadler(
    State(state): State<AppState>,
    Path(session_id): Path<Uuid>,
    Json(request): Json<CreateMessageRequest>,
) -> Response {
    // The client sends conversation history, so the latest message must exist.
    let Some(user_message) = request.input.last().cloned() else {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": {
                    "code": "invalid_message",
                    "message": "input must contain at least one message",
                }
            })),
        )
            .into_response();
    };

    // This endpoint only accepts messages authored by the user.
    if user_message.role != Role::User {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": {
                    "code": "invalid_message",
                    "message": "latest message must be a user message",
                }
            })),
        )
            .into_response();
    }

    // Tool calls and tool outputs are produced by the server-side agent loop.
    let contains_only_user_input = user_message.content.iter().all(|content| {
        matches!(
            content,
            MessageContent::InputText { .. }
                | MessageContent::InputImage(_)
                | MessageContent::InputFile(_)
        )
    });

    if !contains_only_user_input {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": {
                    "code": "invalid_message",
                    "message": "latest user message can only contain input_text, input_image, or input_file",
                }
            })),
        )
        .into_response();
    }

    // 1. Save user message to DB
    let saved_message = match state
        .chat_message_repository
        .append(session_id, user_message)
        .await
    {
        Ok(saved_message) => saved_message,
        Err(ChatRepositoryError::SessionNotFound(_)) => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({
                    "error": {
                        "code": "session_not_found",
                        "message": format!("session not found: {session_id}"),
                    }
                })),
            )
                .into_response();
        }
        Err(err) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": {
                        "code": "failed_to_create_message",
                        "message": err.to_string(),
                    }
                })),
            )
                .into_response();
        }
    };

    // 2. Update session status to running
    match state
        .chat_session_repository
        .update_status(session_id, ChatSessionStatus::Running)
        .await
    {
        Ok(_) => {}
        Err(ChatRepositoryError::SessionNotFound(_)) => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({
                    "error": {
                        "code": "session_not_found",
                        "message": format!("session not found: {session_id}"),
                    }
                })),
            )
                .into_response();
        }
        Err(err) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": {
                        "code": "failed_to_update_session_status",
                        "message": err.to_string(),
                    }
                })),
            )
                .into_response();
        }
    }

    let (tx, rx) = mpsc::channel::<Result<Event, Infallible>>(32);

    tokio::spawn(async move {
        let _ = tx
            .send(Ok(Event::default().event("message").data(
                json!({
                    "status": "started",
                    "message_id": saved_message.id.to_string(),
                })
                .to_string(),
            )))
            .await;

        // 3. Start agent loop
        tokio::time::sleep(Duration::from_millis(500)).await;
        let _ = tx
            .send(Ok(Event::default().event("message").data(
                json!({
                    "role": "assistant",
                    "content": "dummy response",
                })
                .to_string(),
            )))
            .await;

        // 4. Update session status to idle
        match state
            .chat_session_repository
            .update_status(session_id, ChatSessionStatus::Idle)
            .await
        {
            Ok(_) => {
                let _ = tx
                    .send(Ok(Event::default().event("done").data(
                        json!({
                            "status": "done",
                        })
                        .to_string(),
                    )))
                    .await;
            }
            Err(err) => {
                let _ = tx
                    .send(Ok(Event::default().event("error").data(
                        json!({
                            "error": {
                                "code": "failed_to_update_session_status",
                                "message": err.to_string(),
                            }
                        })
                        .to_string(),
                    )))
                    .await;
            }
        }

        // 5. tx is dropped here, so SSE stream ends
    });

    let stream = stream::unfold(rx, |mut rx| async {
        rx.recv().await.map(|event| (event, rx))
    });

    Sse::new(stream)
        .keep_alive(
            KeepAlive::new()
                .interval(Duration::from_secs(15))
                .text("keep-alive"),
        )
        .into_response()
}
