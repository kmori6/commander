use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use crate::application::error::message_usecase_error::MessageUsecaseError;
use crate::domain::error::message_repository_error::MessageRepositoryError;
use crate::domain::model::message::{Message, MessageContent};
use crate::presentation::state::app_state::AppState;

#[derive(Debug, Deserialize)]
pub struct CreateMessageRequest {
    pub text: String,
}

fn content_json(content: MessageContent) -> serde_json::Value {
    match content {
        MessageContent::InputText { text } => json!({
            "type": "input_text",
            "text": text,
        }),
        MessageContent::OutputText { text } => json!({
            "type": "output_text",
            "text": text,
        }),
        MessageContent::ToolCall {
            call_id,
            tool_name,
            arguments,
        } => json!({
            "type": "tool_call",
            "call_id": call_id,
            "tool_name": tool_name,
            "arguments": arguments,
        }),
        MessageContent::ToolCallOutput {
            call_id,
            output,
            status,
        } => json!({
            "type": "tool_call_output",
            "call_id": call_id,
            "output": output,
            "output_status": status.as_str(),
        }),
    }
}

fn message_json(message: Message) -> serde_json::Value {
    json!({
        "id": message.id.to_string(),
        "session_id": message.session_id.to_string(),
        "role": message.role.as_str(),
        "content": message.contents.into_iter().map(content_json).collect::<Vec<_>>(),
        "created_at": message.created_at.to_rfc3339(),
    })
}

pub async fn create_message_handler(
    State(state): State<AppState>,
    Path(session_id): Path<Uuid>,
    Json(request): Json<CreateMessageRequest>,
) -> Response {
    match state
        .message_usecase
        .save_user_text(session_id, request.text)
        .await
    {
        Ok(message) => (
            StatusCode::ACCEPTED,
            Json(json!({
                "message": message_json(message),
                "task_id": null,
            })),
        )
            .into_response(),
        Err(MessageUsecaseError::MessageRepository(MessageRepositoryError::SessionNotFound(_))) => {
            (
                StatusCode::NOT_FOUND,
                Json(json!({
                    "error": {
                        "code": "session_not_found",
                        "message": format!("session not found: {session_id}"),
                    }
                })),
            )
                .into_response()
        }
        Err(MessageUsecaseError::MessageRepository(MessageRepositoryError::InvalidMessage(_))) => (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": {
                    "code": "invalid_message",
                    "message": "invalid message",
                }
            })),
        )
            .into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "error": {
                    "code": "failed_to_create_message",
                    "message": err.to_string(),
                }
            })),
        )
            .into_response(),
    }
}
