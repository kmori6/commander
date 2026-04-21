use crate::domain::model::attachment::Attachment;
use crate::domain::model::role::Role;
use crate::domain::model::tool::ToolCall;
use crate::domain::model::tool::ToolResultMessage;

#[derive(Debug, Clone)]
pub struct Message {
    pub role: Role,
    pub content: MessageContent,
}

#[derive(Debug, Clone)]
pub enum MessageContent {
    Text(String),
    Multimodal {
        text: String,
        attachments: Vec<Attachment>,
    },
    ToolCall {
        text: Option<String>,
        tool_calls: Vec<ToolCall>,
    },
    ToolResults(Vec<ToolResultMessage>),
}

impl Message {
    pub fn text(role: Role, content: impl Into<String>) -> Self {
        Self {
            role,
            content: MessageContent::Text(content.into()),
        }
    }

    pub fn multimodal(role: Role, text: impl Into<String>, attachments: Vec<Attachment>) -> Self {
        Self {
            role,
            content: MessageContent::Multimodal {
                text: text.into(),
                attachments,
            },
        }
    }

    pub fn tool_results(tool_results: Vec<ToolResultMessage>) -> Self {
        Self {
            role: Role::Tool,
            content: MessageContent::ToolResults(tool_results),
        }
    }

    pub fn tool_call(text: Option<String>, tool_calls: Vec<ToolCall>) -> Self {
        Self {
            role: Role::Assistant,
            content: MessageContent::ToolCall { text, tool_calls },
        }
    }
}
