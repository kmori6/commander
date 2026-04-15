use crate::application::error::llm_client_error::LlmClientError;
use crate::domain::model::message::Message;
use async_trait::async_trait;

#[derive(Debug, Clone)]
pub struct LlmRequest {
    pub messages: Vec<Message>,
}

#[derive(Debug, Clone)]
pub struct LlmResponse {
    pub text: String,
}

#[async_trait]
pub trait LlmClient: Send + Sync {
    async fn response(
        &self,
        request: LlmRequest,
        model: &str,
    ) -> Result<LlmResponse, LlmClientError>;
}
