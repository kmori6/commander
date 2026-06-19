use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::domain::error::llm_provider_error::LlmProviderError;
use crate::domain::model::message::{MessageContent, MessageUsage, Role};
use crate::domain::model::tool_call::ToolSpec;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LlmMessage {
    pub role: Role,
    pub contents: Vec<MessageContent>,
}

impl LlmMessage {
    pub fn new(role: Role, contents: Vec<MessageContent>) -> Self {
        Self { role, contents }
    }

    pub fn system_text(text: impl Into<String>) -> Self {
        Self {
            role: Role::System,
            contents: vec![MessageContent::input_text(text)],
        }
    }

    pub fn user_text(text: impl Into<String>) -> Self {
        Self {
            role: Role::User,
            contents: vec![MessageContent::input_text(text)],
        }
    }

    pub fn output_text(&self, separator: &str) -> String {
        self.contents
            .iter()
            .filter_map(|content| match content {
                MessageContent::OutputText { text } => Some(text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join(separator)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LlmRequest {
    pub messages: Vec<LlmMessage>,
    pub tools: Vec<ToolSpec>,
}

impl LlmRequest {
    pub fn new(messages: Vec<LlmMessage>) -> Self {
        Self {
            messages,
            tools: Vec::new(),
        }
    }

    pub fn with_tools(mut self, tools: Vec<ToolSpec>) -> Self {
        self.tools = tools;
        self
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LlmResponse {
    pub message: LlmMessage,
    pub usage: MessageUsage,
}

impl LlmResponse {
    pub fn output_text(&self, separator: &str) -> String {
        self.message.output_text(separator)
    }
}

#[async_trait]
pub trait LlmProvider: Send + Sync {
    async fn response(&self, request: LlmRequest) -> Result<LlmResponse, LlmProviderError>;

    fn model(&self) -> &str;

    fn context_window(&self) -> i64;
}
