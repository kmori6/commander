use crate::application::port::llm_client::LlmRequest;
use crate::application::{
    error::agent_usecase_error::AgentUsecaseError, port::llm_client::LlmClient,
};
use crate::domain::model::message::Message;
use crate::domain::model::role::Role;

const MODEL: &str = "global.anthropic.claude-sonnet-4-6";

#[derive(Debug)]
pub struct HandleAgentInput {
    pub user_input: String,
}

#[derive(Debug)]
pub struct HandleAgentOutput {
    pub reply: Vec<AgentEvent>,
}

#[derive(Debug)]
pub enum AgentEvent {
    AssistantMessage(String),
}

pub struct AgentUsecase<L> {
    llm_client: L,
}

impl<L: LlmClient> AgentUsecase<L> {
    pub fn new(llm_client: L) -> Self {
        Self { llm_client }
    }

    pub async fn handle(
        &self,
        input: HandleAgentInput,
    ) -> Result<HandleAgentOutput, AgentUsecaseError> {
        let response = self
            .llm_client
            .response(
                LlmRequest {
                    messages: vec![Message {
                        role: Role::User,
                        content: input.user_input,
                    }],
                },
                MODEL,
            )
            .await?;

        Ok(HandleAgentOutput {
            reply: vec![AgentEvent::AssistantMessage(response.text)],
        })
    }
}
