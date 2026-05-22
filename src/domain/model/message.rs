use crate::domain::error::message_domain_error::MessageDomainError;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    System,
    User,
    Assistant,
}

impl Role {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::User => "user",
            Self::Assistant => "assistant",
        }
    }

    pub fn from_db(value: &str) -> Option<Self> {
        match value {
            "system" => Some(Self::System),
            "user" => Some(Self::User),
            "assistant" => Some(Self::Assistant),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolCallOutputStatus {
    Success,
    Error,
}

impl ToolCallOutputStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::Error => "error",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MessageContent {
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
        status: ToolCallOutputStatus,
    },
}

impl MessageContent {
    pub fn input_text(text: impl Into<String>) -> Self {
        Self::InputText { text: text.into() }
    }

    pub fn output_text(text: impl Into<String>) -> Self {
        Self::OutputText { text: text.into() }
    }

    pub fn input_image(image_url: impl Into<String>) -> Self {
        Self::InputImage {
            image_url: image_url.into(),
        }
    }

    pub fn input_file(filename: impl Into<String>, file_data: impl Into<String>) -> Self {
        Self::InputFile {
            filename: filename.into(),
            file_data: file_data.into(),
        }
    }

    pub fn can_persist(&self) -> bool {
        !matches!(self, Self::InputImage { .. } | Self::InputFile { .. })
    }

    pub fn fits_role(&self, role: Role) -> bool {
        match self {
            Self::InputText { .. } => matches!(role, Role::System | Role::User),
            Self::InputImage { .. } | Self::InputFile { .. } => role == Role::User,
            Self::OutputText { .. } | Self::ToolCall { .. } => role == Role::Assistant,
            Self::ToolCallOutput { .. } => role == Role::User,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MessageUsage {
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cache_read_tokens: i64,
    pub cache_write_tokens: i64,
}

impl MessageUsage {
    pub fn total_tokens(self) -> i64 {
        self.input_tokens + self.output_tokens
    }

    pub fn is_valid(self) -> bool {
        self.input_tokens >= 0
            && self.output_tokens >= 0
            && self.cache_read_tokens >= 0
            && self.cache_write_tokens >= 0
    }

    pub fn validate(self) -> Result<Self, MessageDomainError> {
        if self.is_valid() {
            Ok(self)
        } else {
            Err(MessageDomainError::InvalidUsage)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskUsage {
    pub task_id: Uuid,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cache_read_tokens: i64,
    pub cache_write_tokens: i64,
}

impl TaskUsage {
    pub fn total_tokens(&self) -> i64 {
        self.input_tokens + self.output_tokens
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NewMessage {
    pub task_id: Uuid,
    pub role: Role,
    pub contents: Vec<MessageContent>,
    pub model: Option<String>,
    pub usage: Option<MessageUsage>,
}

impl NewMessage {
    // user input -> user message
    pub fn user_text(task_id: Uuid, text: impl Into<String>) -> Result<Self, MessageDomainError> {
        Self::user(task_id, vec![MessageContent::input_text(text)])
    }

    // input text only
    pub fn user(task_id: Uuid, contents: Vec<MessageContent>) -> Result<Self, MessageDomainError> {
        validate_user_input_contents(&contents)?;

        Ok(Self {
            task_id,
            role: Role::User,
            contents,
            model: None,
            usage: None,
        })
    }

    // system instruction text
    pub fn system_text(task_id: Uuid, text: impl Into<String>) -> Result<Self, MessageDomainError> {
        let contents = vec![MessageContent::input_text(text)];
        validate_system_contents(&contents)?;

        Ok(Self {
            task_id,
            role: Role::System,
            contents,
            model: None,
            usage: None,
        })
    }

    // assistant response requires model and usage
    pub fn assistant_response(
        task_id: Uuid,
        contents: Vec<MessageContent>,
        model: &str,
        usage: MessageUsage,
    ) -> Result<Self, MessageDomainError> {
        validate_assistant_contents(&contents)?;

        let model = model.trim();
        if model.is_empty() {
            return Err(MessageDomainError::EmptyModel);
        }

        Ok(Self {
            task_id,
            role: Role::Assistant,
            contents,
            model: Some(model.to_string()),
            usage: Some(usage.validate()?),
        })
    }

    // tool call outputs -> user message
    pub fn tool_call_outputs(
        task_id: Uuid,
        contents: Vec<MessageContent>,
    ) -> Result<Self, MessageDomainError> {
        validate_tool_output_contents(&contents)?;

        Ok(Self {
            task_id,
            role: Role::User,
            contents,
            model: None,
            usage: None,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Message {
    pub id: Uuid,
    pub task_id: Uuid,
    pub role: Role,
    pub contents: Vec<MessageContent>,
    pub model: Option<String>,
    pub usage: Option<MessageUsage>,
    pub created_at: DateTime<Utc>,
}

impl Message {
    // persisted row -> domain message
    pub fn restore(
        id: Uuid,
        task_id: Uuid,
        role: Role,
        contents: Vec<MessageContent>,
        model: Option<String>,
        usage: Option<MessageUsage>,
        created_at: DateTime<Utc>,
    ) -> Result<Self, MessageDomainError> {
        validate_persisted_message(role, &contents, model.as_deref(), usage)?;

        Ok(Self {
            id,
            task_id,
            role,
            contents,
            model,
            usage,
            created_at,
        })
    }

    pub fn new_user_text(
        task_id: Uuid,
        text: impl Into<String>,
    ) -> Result<NewMessage, MessageDomainError> {
        NewMessage::user_text(task_id, text)
    }

    pub fn new_assistant_response(
        task_id: Uuid,
        contents: Vec<MessageContent>,
        model: &str,
        usage: MessageUsage,
    ) -> Result<NewMessage, MessageDomainError> {
        NewMessage::assistant_response(task_id, contents, model, usage)
    }

    pub fn new_tool_call_outputs(
        task_id: Uuid,
        contents: Vec<MessageContent>,
    ) -> Result<NewMessage, MessageDomainError> {
        NewMessage::tool_call_outputs(task_id, contents)
    }
}

fn validate_persisted_message(
    role: Role,
    contents: &[MessageContent],
    model: Option<&str>,
    usage: Option<MessageUsage>,
) -> Result<(), MessageDomainError> {
    match role {
        Role::System => {
            validate_system_contents(contents)?;
            validate_no_response_metadata(model, usage)?;
        }
        Role::User => {
            if contents
                .iter()
                .all(|content| matches!(content, MessageContent::InputText { .. }))
            {
                validate_user_input_contents(contents)?;
            } else {
                validate_tool_output_contents(contents)?;
            }
            validate_no_response_metadata(model, usage)?;
        }
        Role::Assistant => {
            validate_assistant_contents(contents)?;
            validate_response_metadata(model, usage)?;
        }
    }

    Ok(())
}

fn validate_system_contents(contents: &[MessageContent]) -> Result<(), MessageDomainError> {
    validate_base_contents(contents)?;

    if contents
        .iter()
        .all(|content| matches!(content, MessageContent::InputText { .. }))
    {
        Ok(())
    } else {
        Err(MessageDomainError::ContentRoleMismatch { role: Role::System })
    }
}

fn validate_user_input_contents(contents: &[MessageContent]) -> Result<(), MessageDomainError> {
    validate_base_contents(contents)?;

    if contents
        .iter()
        .all(|content| matches!(content, MessageContent::InputText { .. }))
    {
        Ok(())
    } else {
        Err(MessageDomainError::ContentRoleMismatch { role: Role::User })
    }
}

fn validate_assistant_contents(contents: &[MessageContent]) -> Result<(), MessageDomainError> {
    validate_base_contents(contents)?;

    if contents.iter().all(|content| {
        matches!(
            content,
            MessageContent::OutputText { .. } | MessageContent::ToolCall { .. }
        )
    }) {
        Ok(())
    } else {
        Err(MessageDomainError::ContentRoleMismatch {
            role: Role::Assistant,
        })
    }
}

fn validate_tool_output_contents(contents: &[MessageContent]) -> Result<(), MessageDomainError> {
    validate_base_contents(contents)?;

    if contents
        .iter()
        .all(|content| matches!(content, MessageContent::ToolCallOutput { .. }))
    {
        Ok(())
    } else {
        Err(MessageDomainError::ContentRoleMismatch { role: Role::User })
    }
}

fn validate_base_contents(contents: &[MessageContent]) -> Result<(), MessageDomainError> {
    if contents.is_empty() {
        return Err(MessageDomainError::EmptyContents);
    }

    for content in contents {
        if !content.can_persist() {
            return Err(MessageDomainError::RuntimeOnlyContent);
        }

        validate_content_fields(content)?;
    }

    Ok(())
}

fn validate_content_fields(content: &MessageContent) -> Result<(), MessageDomainError> {
    match content {
        MessageContent::InputText { text } | MessageContent::OutputText { text } => {
            reject_blank("text", text)
        }
        MessageContent::InputImage { image_url } => reject_blank("image_url", image_url),
        MessageContent::InputFile {
            filename,
            file_data,
        } => {
            reject_blank("filename", filename)?;
            reject_blank("file_data", file_data)
        }
        MessageContent::ToolCall {
            call_id, tool_name, ..
        } => {
            reject_blank("call_id", call_id)?;
            reject_blank("tool_name", tool_name)
        }
        MessageContent::ToolCallOutput { call_id, .. } => reject_blank("call_id", call_id),
    }
}

fn reject_blank(field: &str, value: &str) -> Result<(), MessageDomainError> {
    if value.trim().is_empty() {
        return Err(MessageDomainError::InvalidContent(format!(
            "{field} must not be empty"
        )));
    }

    Ok(())
}

fn validate_response_metadata(
    model: Option<&str>,
    usage: Option<MessageUsage>,
) -> Result<(), MessageDomainError> {
    match model {
        Some(model) if !model.trim().is_empty() => {}
        Some(_) => return Err(MessageDomainError::EmptyModel),
        None => return Err(MessageDomainError::MissingModel),
    }

    usage
        .ok_or(MessageDomainError::MissingUsage)?
        .validate()
        .map(|_| ())
}

fn validate_no_response_metadata(
    model: Option<&str>,
    usage: Option<MessageUsage>,
) -> Result<(), MessageDomainError> {
    if model.is_some() || usage.is_some() {
        return Err(MessageDomainError::UnexpectedResponseMetadata);
    }

    Ok(())
}
