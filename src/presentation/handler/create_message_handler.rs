use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::application::error::message_usecase_error::MessageUsecaseError;
use crate::domain::model::message::{Message, MessageContent};
use crate::presentation::state::app_state::AppState;

#[derive(Debug, Deserialize)]
pub struct CreateMessageRequest {
    pub text: String,

    #[serde(default)]
    pub input_images: Vec<Value>,

    #[serde(default)]
    pub input_files: Vec<Value>,
}

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
        "task_id": message.task_id.to_string(),
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
    if !request.input_images.is_empty() || !request.input_files.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": {
                    "code": "attachments_not_supported",
                    "message": "chat image/file attachments are not supported; place files in the workspace and reference their paths",
                }
            })),
        )
            .into_response();
    }

    let text = request.text;

    match state.message_usecase.save_user_text(session_id, text).await {
        Ok(message_task) => {
            let task_id = message_task.task.id;

            (
                StatusCode::ACCEPTED,
                Json(json!({
                    "message": message_json(message_task.message),
                    "task_id": task_id.to_string(),
                })),
            )
                .into_response()
        }
        Err(MessageUsecaseError::MessageRepository(_)) => (
            StatusCode::NOT_FOUND,
            Json(json!({
                "error": {
                    "code": "session_not_found",
                    "message": format!("session not found: {session_id}"),
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
