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
    pub fn is_valid(self) -> bool {
        self.input_tokens >= 0
            && self.output_tokens >= 0
            && self.cache_read_tokens >= 0
            && self.cache_write_tokens >= 0
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
pub struct Message {
    pub id: Uuid,
    pub task_id: Uuid,
    pub role: Role,
    pub contents: Vec<MessageContent>,
    pub model: Option<String>,
    pub usage: Option<MessageUsage>,
    pub created_at: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn usage_rejects_negative_tokens() {
        let usage = MessageUsage {
            input_tokens: 0,
            output_tokens: -1,
            cache_read_tokens: 0,
            cache_write_tokens: 0,
        };

        assert!(!usage.is_valid());
    }

    #[test]
    fn tool_call_fits_assistant_only() {
        let content = MessageContent::ToolCall {
            call_id: "call_1".to_string(),
            tool_name: "shell".to_string(),
            arguments: json!({}),
        };

        assert!(content.fits_role(Role::Assistant));
        assert!(!content.fits_role(Role::User));
        assert!(!content.fits_role(Role::System));
    }

    #[test]
    fn tool_output_fits_user_only() {
        let content = MessageContent::ToolCallOutput {
            call_id: "call_1".to_string(),
            output: json!({ "ok": true }),
            status: ToolCallOutputStatus::Success,
        };

        assert!(content.fits_role(Role::User));
        assert!(!content.fits_role(Role::Assistant));
        assert!(!content.fits_role(Role::System));
    }

    #[test]
    fn file_content_fits_user_only() {
        let content = MessageContent::input_file("a.txt", "data:text/plain;base64,xxx");

        assert!(content.fits_role(Role::User));
        assert!(!content.fits_role(Role::Assistant));
        assert!(!content.fits_role(Role::System));
    }
}
