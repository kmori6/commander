use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::domain::error::llm_provider_error::LlmProviderError;
use crate::domain::model::message::{MessageContent, Role};
use crate::domain::model::token_usage::TokenUsageCounts;
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

    pub fn assistant_text(text: impl Into<String>) -> Self {
        Self {
            role: Role::Assistant,
            contents: vec![MessageContent::output_text(text)],
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

    pub fn has_tool_calls(&self) -> bool {
        self.contents
            .iter()
            .any(|content| matches!(content, MessageContent::ToolCall { .. }))
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StructuredOutputSchema {
    pub name: String,
    pub description: Option<String>,
    pub schema: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LlmRequest {
    pub model: String,
    pub messages: Vec<LlmMessage>,
    pub tools: Vec<ToolSpec>,
    pub structured_output: Option<StructuredOutputSchema>,
}

impl LlmRequest {
    pub fn new(model: impl Into<String>, messages: Vec<LlmMessage>) -> Self {
        Self {
            model: model.into(),
            messages,
            tools: Vec::new(),
            structured_output: None,
        }
    }

    pub fn with_tools(mut self, tools: Vec<ToolSpec>) -> Self {
        self.tools = tools;
        self
    }

    pub fn with_structured_output(mut self, schema: StructuredOutputSchema) -> Self {
        self.structured_output = Some(schema);
        self
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LlmResponse {
    pub message: LlmMessage,
    pub usage: TokenUsageCounts,
}

impl LlmResponse {
    pub fn output_text(&self, separator: &str) -> String {
        self.message.output_text(separator)
    }
}

#[async_trait]
pub trait LlmProvider: Send + Sync {
    async fn respond(&self, request: LlmRequest) -> Result<LlmResponse, LlmProviderError>;
}
