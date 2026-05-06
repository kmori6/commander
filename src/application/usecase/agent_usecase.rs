use crate::application::error::agent_usecase_error::AgentUsecaseError;
use crate::application::runtime::agent_runtime::AgentRuntime;
use crate::domain::model::app_event::AppEvent;
use crate::domain::model::chat_message::ChatMessage;
use crate::domain::model::chat_session::ChatSession;
use crate::domain::model::input_file::InputFile;
use crate::domain::model::input_image::InputImage;
use crate::domain::model::message::{Message, MessageContent};
use crate::domain::model::tool_approval::ToolApprovalResponse;
use crate::domain::port::llm_provider::LlmProvider;
use crate::domain::repository::awaiting_tool_approval_repository::AwaitingToolApprovalRepository;
use crate::domain::repository::chat_message_repository::ChatMessageRepository;
use crate::domain::repository::chat_session_repository::ChatSessionRepository;
use crate::domain::repository::token_usage_repository::TokenUsageRepository;
use crate::domain::repository::tool_approval_repository::ToolApprovalRepository;
use tokio::sync::mpsc;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub enum Attachment {
    Image(InputImage),
    File(InputFile),
}

#[derive(Debug)]
pub struct AgentStartTurnOutput {
    pub events: Vec<AppEvent>,
}

pub struct AgentUsecase<L, S, M, T, A, W> {
    agent_runtime: AgentRuntime<L, S, M, T, A, W>,
    chat_session_repository: S,
    chat_message_repository: M,
}

pub struct AgentUsecaseRepositories<S, M, T, A, W> {
    pub chat_session_repository: S,
    pub chat_message_repository: M,
    pub token_usage_repository: T,
    pub tool_approval_repository: A,
    pub awaiting_tool_approval_repository: W,
}

impl<L, S, M, T, A, W> AgentUsecase<L, S, M, T, A, W>
where
    L: LlmProvider,
    S: ChatSessionRepository,
    M: ChatMessageRepository,
    T: TokenUsageRepository,
    A: ToolApprovalRepository,
    W: AwaitingToolApprovalRepository,
{
    pub fn new(
        agent_runtime: AgentRuntime<L, S, M, T, A, W>,
        repositories: AgentUsecaseRepositories<S, M, T, A, W>,
    ) -> Self {
        Self {
            agent_runtime,
            chat_session_repository: repositories.chat_session_repository,
            chat_message_repository: repositories.chat_message_repository,
        }
    }

    pub async fn accept_message(
        &self,
        session_id: Uuid,
        job_run_id: Option<Uuid>,
        user_message: Message,
    ) -> Result<ChatMessage, AgentUsecaseError> {
        user_message.validate_user_input()?;

        let session = self
            .chat_session_repository
            .find_by_id(session_id)
            .await?
            .ok_or(AgentUsecaseError::SessionNotFound(session_id))?;

        let title = if session.title.is_none() {
            let summaries = self
                .chat_message_repository
                .summarize_by_session_ids(&[session_id])
                .await?;

            let has_messages = summaries
                .first()
                .is_some_and(|summary| summary.message_count > 0);

            if has_messages {
                None
            } else {
                user_message
                    .content
                    .iter()
                    .find_map(|content| match content {
                        MessageContent::InputText { text } => {
                            ChatSession::title_from_first_user_message(text)
                        }
                        _ => None,
                    })
            }
        } else {
            None
        };

        let next_status = session.start_turn()?;

        self.chat_session_repository
            .update_status(session_id, next_status)
            .await?;

        let saved_user_message = self
            .chat_message_repository
            .append(session_id, job_run_id, user_message)
            .await?;

        if let Some(title) = title
            && let Err(err) = self
                .chat_session_repository
                .update_title(session_id, title)
                .await
        {
            log::warn!("failed to update chat session title for session {session_id}: {err}");
        }

        Ok(saved_user_message)
    }

    pub async fn run_turn(
        &self,
        session_id: Uuid,
        user_message: ChatMessage,
        tx: mpsc::Sender<AppEvent>,
    ) -> Result<AgentStartTurnOutput, AgentUsecaseError> {
        let output = self
            .agent_runtime
            .run_turn(session_id, user_message, tx)
            .await?;

        Ok(AgentStartTurnOutput {
            events: output.events,
        })
    }

    pub async fn resolve_approval(
        &self,
        session_id: Uuid,
        decision: ToolApprovalResponse,
        tx: mpsc::Sender<AppEvent>,
    ) -> Result<AgentStartTurnOutput, AgentUsecaseError> {
        let output = self
            .agent_runtime
            .resume_turn(session_id, decision, tx)
            .await?;

        Ok(AgentStartTurnOutput {
            events: output.events,
        })
    }
}
