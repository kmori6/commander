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
use crate::domain::model::message::{Message, MessageContent};
use crate::presentation::state::app_state::AppState;

#[derive(Debug, Deserialize)]
pub struct CreateMessageRequest {
    pub text: String,

    #[serde(default)]
    pub input_images: Vec<CreateInputImage>,

    #[serde(default)]
    pub input_files: Vec<CreateInputFile>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateInputImage {
    pub image_url: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateInputFile {
    pub filename: String,
    pub file_data: String,
}

fn runtime_contents(request: &CreateMessageRequest) -> Vec<MessageContent> {
    let mut contents = vec![MessageContent::input_text(request.text.clone())];

    contents.extend(
        request
            .input_images
            .iter()
            .map(|image| MessageContent::input_image(image.image_url.clone())),
    );

    contents.extend(
        request
            .input_files
            .iter()
            .map(|file| MessageContent::input_file(file.filename.clone(), file.file_data.clone())),
    );

    contents
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
    let user_contents = runtime_contents(&request);
    let text = request.text;

    match state.message_usecase.save_user_text(session_id, text).await {
        Ok(message_task) => {
            let task_id = message_task.task.id;
            let agent_runtime = state.agent_runtime.clone();

            tokio::spawn(async move {
                if let Err(err) = agent_runtime.run(task_id, Some(user_contents)).await {
                    log::warn!("failed to run task {task_id}: {err}");
                }
            });

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
