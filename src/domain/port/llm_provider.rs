use crate::domain::error::llm_provider_error::LlmProviderError;
use crate::domain::model::message::{Message, MessageContent};
use crate::domain::model::token_usage::TokenUsage;
use crate::domain::model::tool_call::ToolSpec;
use async_trait::async_trait;
use serde_json::Value;

#[derive(Debug, Clone)]
pub struct LlmResponse {
    pub message: Message,
    pub usage: TokenUsage,
}

impl LlmResponse {
    pub fn output_text(&self, separator: &str) -> String {
        self.message
            .content
            .iter()
            .filter_map(|content| match content {
                MessageContent::OutputText { text } => Some(text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join(separator)
    }
}

#[derive(Debug, Clone)]
pub struct StructuredOutputSchema {
    pub name: String,
    pub description: Option<String>,
    pub schema: serde_json::Value,
}

#[async_trait]
pub trait LlmProvider: Send + Sync {
    async fn response(
        &self,
        messages: Vec<Message>,
        model: &str,
    ) -> Result<LlmResponse, LlmProviderError>;

    async fn response_with_tool(
        &self,
        messages: Vec<Message>,
        tools: Vec<ToolSpec>,
        model: &str,
    ) -> Result<LlmResponse, LlmProviderError>;

    async fn response_with_structure(
        &self,
        messages: Vec<Message>,
        schema: StructuredOutputSchema,
        model: &str,
    ) -> Result<Value, LlmProviderError>;
}
