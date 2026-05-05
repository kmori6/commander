use crate::domain::error::message_error::MessageError;
use crate::domain::model::input_file::InputFile;
use crate::domain::model::input_image::InputImage;
use crate::domain::model::role::Role;
use crate::domain::model::tool_call::ToolCall;
use crate::domain::model::tool_call_output::ToolCallOutput;
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

    /// Tool calls are actionable requests emitted by the assistant.
    pub fn tool_calls(&self) -> Vec<ToolCall> {
        self.content
            .iter()
            .filter_map(|content| match content {
                MessageContent::ToolCall(call) => Some(call.clone()),
                _ => None,
            })
            .collect()
    }

    pub fn assistant_tool_calls(tool_calls: Vec<ToolCall>) -> Result<Self, MessageError> {
        Self::new(
            Role::Assistant,
            tool_calls
                .into_iter()
                .map(MessageContent::ToolCall)
                .collect(),
        )
    }

    pub fn user_tool_call_outputs(outputs: Vec<ToolCallOutput>) -> Result<Self, MessageError> {
        Self::new(
            Role::User,
            outputs
                .into_iter()
                .map(MessageContent::ToolCallOutput)
                .collect(),
        )
    }

    /// A user input message must be user-authored, include text, and contain only input blocks.
    pub fn validate_user_input(&self) -> Result<(), MessageError> {
        if self.role != Role::User {
            return Err(MessageError::InvalidContent(
                "message role must be user".to_string(),
            ));
        }

        let has_input_text = self
            .content
            .iter()
            .any(|content| matches!(content, MessageContent::InputText { .. }));

        if !has_input_text {
            return Err(MessageError::InvalidContent(
                "user message must contain input_text".to_string(),
            ));
        }

        let contains_only_user_input = self.content.iter().all(|content| {
            matches!(
                content,
                MessageContent::InputText { .. }
                    | MessageContent::InputImage(_)
                    | MessageContent::InputFile(_)
            )
        });

        if !contains_only_user_input {
            return Err(MessageError::InvalidContent(
                "user message can only contain input_text, input_image, or input_file".to_string(),
            ));
        }

        Ok(())
    }

    /// Assistant text is exposed as non-empty output blocks.
    pub fn output_texts(&self) -> Vec<String> {
        self.content
            .iter()
            .filter_map(|content| match content {
                MessageContent::OutputText { text } if !text.is_empty() => Some(text.clone()),
                _ => None,
            })
            .collect()
    }

    /// Approval resume restores the original assistant tool call by id.
    pub fn find_tool_call(&self, call_id: &str) -> Option<ToolCall> {
        self.tool_calls()
            .into_iter()
            .find(|call| call.call_id == call_id)
    }
}
