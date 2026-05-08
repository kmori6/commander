use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde_json::json;
use uuid::Uuid;

use crate::domain::model::message::{Message, MessageContent};
use crate::presentation::state::app_state::AppState;

fn content_json(content: MessageContent) -> serde_json::Value {
    match content {
        MessageContent::InputText { text } => json!({
            "type": "input_text",
            "text": text,
        }),
        MessageContent::InputImage { image_url } => json!({
            "type": "input_image",
            "image_url": image_url,
        }),
        MessageContent::InputFile {
            filename,
            file_data,
        } => json!({
            "type": "input_file",
            "filename": filename,
            "file_data": file_data,
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

pub async fn list_message_handler(
    State(state): State<AppState>,
    Path(session_id): Path<Uuid>,
) -> Response {
    match state.message_usecase.list_for_session(session_id).await {
        Ok(messages) => (
            StatusCode::OK,
            Json(json!({
                "messages": messages.into_iter().map(message_json).collect::<Vec<_>>(),
            })),
        )
            .into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "error": {
                    "code": "failed_to_list_messages",
                    "message": err.to_string(),
                }
            })),
        )
            .into_response(),
    }
}
