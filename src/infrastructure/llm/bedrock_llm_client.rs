use crate::application::error::llm_client_error::LlmClientError;
use crate::application::port::llm_client::{LlmClient, LlmRequest, LlmResponse};
use crate::domain::model::role::Role;
use async_trait::async_trait;
use aws_sdk_bedrockruntime::{
    Client,
    types::{ContentBlock, ConversationRole, Message, SystemContentBlock},
};

pub struct BedrockLlmClient {
    client: Client,
}

impl BedrockLlmClient {
    pub fn new(client: Client) -> Self {
        Self { client }
    }

    pub async fn from_default_config() -> Result<Self, LlmClientError> {
        let config = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
        let client = Client::new(&config);
        Ok(Self { client })
    }
}

#[async_trait]
impl LlmClient for BedrockLlmClient {
    async fn response(
        &self,
        request: LlmRequest,
        model: &str,
    ) -> Result<LlmResponse, LlmClientError> {
        let system_blocks: Vec<SystemContentBlock> = request
            .messages
            .iter()
            .filter(|m| m.role == Role::System)
            .map(|m| SystemContentBlock::Text(m.content.clone()))
            .collect();

        let mut message_blocks: Vec<Message> = vec![];
        for m in request.messages.iter().filter(|m| m.role != Role::System) {
            let role = match m.role {
                Role::Assistant => ConversationRole::Assistant,
                _ => ConversationRole::User,
            };
            let msg = Message::builder()
                .role(role)
                .content(ContentBlock::Text(m.content.clone()))
                .build()
                .map_err(|e| {
                    LlmClientError::RequestBuild(format!("Error building Bedrock message: {}", e))
                })?;
            message_blocks.push(msg);
        }

        let mut req = self
            .client
            .converse()
            .model_id(model)
            .set_messages(Some(message_blocks));

        for block in system_blocks {
            req = req.system(block);
        }

        let output = req
            .send()
            .await
            .map_err(|e| LlmClientError::ApiCall(format!("Bedrock converse error: {}", e)))?;

        let output_blocks = output
            .output()
            .ok_or_else(|| {
                LlmClientError::ResponseParse("No output in Bedrock response".to_string())
            })?
            .as_message()
            .map_err(|_| {
                LlmClientError::ResponseParse(
                    "Unsupported output type in Bedrock response".to_string(),
                )
            })?
            .content();

        let text = output_blocks
            .first()
            .ok_or_else(|| {
                LlmClientError::ResponseParse("No content blocks in Bedrock response".to_string())
            })?
            .as_text()
            .map_err(|_| {
                LlmClientError::ResponseParse(
                    "Unsupported content block type in Bedrock response".to_string(),
                )
            })?
            .to_string();

        Ok(LlmResponse { text })
    }
}
