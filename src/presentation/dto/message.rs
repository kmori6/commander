use serde::Serialize;
use serde_json::Value;

use crate::domain::model::message::{Message, MessageContent, TaskUsage};

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MessageContentResponse {
    InputText {
        text: String,
    },
    InputImage {
        image_url: String,
    },
    InputFile {
        filename: String,
        file_data: String,
    },
    OutputText {
        text: String,
    },
    ToolCall {
        call_id: String,
        tool_name: String,
        arguments: Value,
    },
    ToolCallOutput {
        call_id: String,
        output: Value,
        output_status: String,
    },
}

impl From<MessageContent> for MessageContentResponse {
    fn from(content: MessageContent) -> Self {
        match content {
            MessageContent::InputText { text } => Self::InputText { text },
            MessageContent::InputImage { image_url } => Self::InputImage { image_url },
            MessageContent::InputFile {
                filename,
                file_data,
            } => Self::InputFile {
                filename,
                file_data,
            },
            MessageContent::OutputText { text } => Self::OutputText { text },
            MessageContent::ToolCall {
                call_id,
                tool_name,
                arguments,
            } => Self::ToolCall {
                call_id,
                tool_name,
                arguments,
            },
            MessageContent::ToolCallOutput {
                call_id,
                output,
                status,
            } => Self::ToolCallOutput {
                call_id,
                output,
                output_status: status.as_str().to_string(),
            },
        }
    }
}

#[derive(Debug, Serialize)]
pub struct MessageResponse {
    pub id: String,
    pub task_id: String,
    pub role: String,
    pub content: Vec<MessageContentResponse>,
    pub created_at: String,
}

impl From<Message> for MessageResponse {
    fn from(message: Message) -> Self {
        Self {
            id: message.id.to_string(),
            task_id: message.task_id.to_string(),
            role: message.role.as_str().to_string(),
            content: message.contents.into_iter().map(Into::into).collect(),
            created_at: message.created_at.to_rfc3339(),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct TaskUsageResponse {
    pub task_id: String,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cache_read_tokens: i64,
    pub cache_write_tokens: i64,
    pub total_tokens: i64,
}

impl From<TaskUsage> for TaskUsageResponse {
    fn from(usage: TaskUsage) -> Self {
        Self {
            task_id: usage.task_id.to_string(),
            input_tokens: usage.input_tokens,
            output_tokens: usage.output_tokens,
            cache_read_tokens: usage.cache_read_tokens,
            cache_write_tokens: usage.cache_write_tokens,
            total_tokens: usage.total_tokens(),
        }
    }
}
