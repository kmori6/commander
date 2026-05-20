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

    pub fn is_persistable(&self) -> bool {
        !matches!(self, Self::InputImage { .. } | Self::InputFile { .. })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Message {
    pub id: Uuid,
    pub task_id: Uuid,
    pub role: Role,
    pub contents: Vec<MessageContent>,
    pub created_at: DateTime<Utc>,
}

impl Message {
    pub fn new(
        id: Uuid,
        task_id: Uuid,
        role: Role,
        contents: Vec<MessageContent>,
        created_at: DateTime<Utc>,
    ) -> Self {
        Self {
            id,
            task_id,
            role,
            contents,
            created_at,
        }
    }
}
