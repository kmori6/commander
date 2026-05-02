use crate::domain::error::message_error::MessageError;
use crate::domain::model::input_file::InputFile;
use crate::domain::model::input_image::InputImage;
use crate::domain::model::role::Role;
use crate::domain::model::tool_call::{ToolCall, ToolCallOutput};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MessageContent {
    InputText { text: String },
    InputImage(InputImage),
    InputFile(InputFile),
    OutputText { text: String },
    ToolCall(ToolCall),
    ToolCallOutput(ToolCallOutput),
}

impl MessageContent {
    pub fn is_persistable(&self) -> bool {
        !matches!(self, Self::InputImage(_) | Self::InputFile(_))
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    pub content: Vec<MessageContent>,
}

impl Message {
    pub fn new(role: Role, content: Vec<MessageContent>) -> Result<Self, MessageError> {
        if content.is_empty() {
            return Err(MessageError::EmptyContents);
        }

        Ok(Self { role, content })
    }

    pub fn input_text(text: impl Into<String>) -> Result<Self, MessageError> {
        Self::new(
            Role::User,
            vec![MessageContent::InputText { text: text.into() }],
        )
    }

    pub fn output_text(text: impl Into<String>) -> Result<Self, MessageError> {
        Self::new(
            Role::Assistant,
            vec![MessageContent::OutputText { text: text.into() }],
        )
    }

    pub fn tool_calls(tool_calls: Vec<ToolCall>) -> Result<Self, MessageError> {
        Self::new(
            Role::Assistant,
            tool_calls
                .into_iter()
                .map(MessageContent::ToolCall)
                .collect(),
        )
    }

    pub fn tool_call_outputs(outputs: Vec<ToolCallOutput>) -> Result<Self, MessageError> {
        Self::new(
            Role::User,
            outputs
                .into_iter()
                .map(MessageContent::ToolCallOutput)
                .collect(),
        )
    }
}
