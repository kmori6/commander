use crate::application::error::agent_usecase_error::AgentUsecaseError;
use crate::domain::model::chat_session::ChatSession;
use crate::domain::model::message::Message;
use crate::domain::model::role::Role;
use crate::domain::port::llm_provider::LlmProvider;
use crate::domain::repository::chat_message_repository::ChatMessageRepository;
use crate::domain::repository::chat_session_repository::ChatSessionRepository;
use crate::domain::service::agent_service::{AgentProgressEvent, AgentService};
use uuid::Uuid;

#[derive(Debug)]
pub struct HandleAgentInput {
    pub session_id: Uuid,
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

pub struct AgentUsecase<L, S, M> {
    agent_service: AgentService<L>,
    chat_session_repository: S,
    chat_message_repository: M,
}

impl<L, S, M> AgentUsecase<L, S, M>
where
    L: LlmProvider,
    S: ChatSessionRepository,
    M: ChatMessageRepository,
{
    pub fn new(
        agent_service: AgentService<L>,
        chat_session_repository: S,
        chat_message_repository: M,
    ) -> Self {
        Self {
            agent_service,
            chat_session_repository,
            chat_message_repository,
        }
    }

    pub async fn start_session(&self) -> Result<ChatSession, AgentUsecaseError> {
        self.chat_session_repository
            .create()
            .await
            .map_err(Into::into)
    }

    pub async fn find_session(
        &self,
        session_id: Uuid,
    ) -> Result<Option<ChatSession>, AgentUsecaseError> {
        self.chat_session_repository
            .find_by_id(session_id)
            .await
            .map_err(Into::into)
    }

    pub async fn list_sessions(&self, limit: usize) -> Result<Vec<ChatSession>, AgentUsecaseError> {
        self.chat_session_repository
            .list_recent(limit)
            .await
            .map_err(Into::into)
    }

    pub async fn handle(
        &self,
        input: HandleAgentInput,
    ) -> Result<HandleAgentOutput, AgentUsecaseError> {
        let history = self
            .chat_message_repository
            .list_for_session(input.session_id)
            .await?
            .into_iter()
            .map(|entry| entry.message)
            .collect::<Vec<Message>>();

        self.chat_message_repository
            .append(
                input.session_id,
                Message::text(Role::User, input.user_input.clone()),
            )
            .await?;

        let result = self
            .agent_service
            .run_with_progress(history, input.user_input, |_| {})
            .await?;

        for message in result.messages {
            self.chat_message_repository
                .append(input.session_id, message)
                .await?;
        }

        let final_text = result.final_text.clone();
        Ok(HandleAgentOutput {
            reply: vec![AgentEvent::AssistantMessage(final_text)],
        })
    }

    pub async fn handle_with_progress<F>(
        &self,
        input: HandleAgentInput,
        emit: F,
    ) -> Result<HandleAgentOutput, AgentUsecaseError>
    where
        F: FnMut(AgentProgressEvent),
    {
        let history = self
            .chat_message_repository
            .list_for_session(input.session_id)
            .await?
            .into_iter()
            .map(|entry| entry.message)
            .collect::<Vec<Message>>();

        self.chat_message_repository
            .append(
                input.session_id,
                Message::text(Role::User, input.user_input.clone()),
            )
            .await?;

        let result = self
            .agent_service
            .run_with_progress(history, input.user_input, emit)
            .await?;

        for message in result.messages {
            self.chat_message_repository
                .append(input.session_id, message)
                .await?;
        }

        let final_text = result.final_text.clone();
        Ok(HandleAgentOutput {
            reply: vec![AgentEvent::AssistantMessage(final_text)],
        })
    }
}
