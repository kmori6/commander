use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::domain::model::chat_message::ChatMessage;
use crate::domain::model::message::MessageContent;
use crate::domain::model::role::Role;
use crate::domain::repository::chat_message_repository::ChatMessageRepository;
use crate::domain::repository::chat_session_repository::ChatSessionRepository;
use crate::presentation::state::app_state::AppState;

#[derive(Debug, Deserialize)]
pub struct ListMessageQuery {
    pub limit: Option<usize>,
}

pub async fn list_message_handler(
    State(state): State<AppState>,
    Path(session_id): Path<Uuid>,
    Query(query): Query<ListMessageQuery>,
) -> Response {
    match state.chat_session_repository.find_by_id(session_id).await {
        Ok(Some(_)) => {}
        Ok(None) => {
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
                        "code": "failed_to_get_session_messages",
                        "message": err.to_string(),
                    }
                })),
            )
                .into_response();
        }
    }

    let messages = match state
        .chat_message_repository
        .list_for_session(session_id)
        .await
    {
        Ok(messages) => messages,
        Err(err) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": {
                        "code": "failed_to_get_session_messages",
                        "message": err.to_string(),
                    }
                })),
            )
                .into_response();
        }
    };

    let messages = apply_limit(messages, query.limit)
        .into_iter()
        .map(message_to_json)
        .collect::<Vec<_>>();

    (
        StatusCode::OK,
        Json(json!({
            "session_id": session_id.to_string(),
            "messages": messages,
        })),
    )
        .into_response()
}

fn role_as_str(role: Role) -> &'static str {
    match role {
        Role::System => "system",
        Role::User => "user",
        Role::Assistant => "assistant",
        Role::Tool => "tool",
    }
}

fn message_to_json(chat_message: ChatMessage) -> Value {
    let mut value = match &chat_message.message.content {
        MessageContent::Text(text) | MessageContent::Multimodal { text, .. } => json!({
            "id": chat_message.id.to_string(),
            "role": role_as_str(chat_message.message.role),
            "kind": "text",
            "text": text,
            "created_at": chat_message.created_at.to_rfc3339(),
        }),
        MessageContent::ToolCall { text, tool_calls } => json!({
            "id": chat_message.id.to_string(),
            "role": role_as_str(chat_message.message.role),
            "kind": "tool_call",
            "text": text,
            "tool_calls": tool_calls.iter().map(|call| {
                json!({
                    "id": call.id,
                    "name": call.name,
                    "arguments": call.arguments,
                })
            }).collect::<Vec<_>>(),
            "created_at": chat_message.created_at.to_rfc3339(),
        }),
        MessageContent::ToolResults(tool_results) => json!({
            "id": chat_message.id.to_string(),
            "role": role_as_str(chat_message.message.role),
            "kind": "tool_results",
            "tool_results": tool_results.iter().map(|result| {
                json!({
                    "tool_call_id": result.tool_call_id,
                    "output": result.output,
                    "is_error": result.is_error,
                })
            }).collect::<Vec<_>>(),
            "created_at": chat_message.created_at.to_rfc3339(),
        }),
    };

    remove_null_text(&mut value);
    value
}

fn remove_null_text(value: &mut Value) {
    if value.get("text").is_some_and(Value::is_null)
        && let Some(object) = value.as_object_mut()
    {
        object.remove("text");
    }
}

fn apply_limit(messages: Vec<ChatMessage>, limit: Option<usize>) -> Vec<ChatMessage> {
    let Some(limit) = limit else {
        return messages;
    };

    let skip = messages.len().saturating_sub(limit);
    messages.into_iter().skip(skip).collect()
}
