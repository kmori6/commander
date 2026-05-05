use crate::application::error::agent_usecase_error::AgentUsecaseError;
use crate::domain::error::agent_error::AgentError;
use crate::domain::error::chat_session_error::ChatSessionError;
use crate::domain::model::app_event::AppEvent;
use crate::domain::model::awaiting_tool_approval::AwaitingToolApproval;
use crate::domain::model::chat_message::ChatMessage;
use crate::domain::model::chat_session::ChatSession;
use crate::domain::model::input_file::InputFile;
use crate::domain::model::input_image::InputImage;
use crate::domain::model::loop_safety::LoopSafety;
use crate::domain::model::message::{Message, MessageContent};
use crate::domain::model::role::Role;
use crate::domain::model::tool_approval::{ToolApproval, ToolApprovalResponse};
use crate::domain::model::tool_call::ToolCall;
use crate::domain::model::tool_call_output::ToolCallOutput;
use crate::domain::model::tool_execution_decision::ToolExecutionDecision;
use crate::domain::port::llm_provider::{LlmProvider, LlmResponse};
use crate::domain::repository::awaiting_tool_approval_repository::AwaitingToolApprovalRepository;
use crate::domain::repository::chat_message_repository::ChatMessageRepository;
use crate::domain::repository::chat_session_repository::ChatSessionRepository;
use crate::domain::repository::token_usage_repository::TokenUsageRepository;
use crate::domain::repository::tool_approval_repository::ToolApprovalRepository;
use crate::domain::service::agent_service::AgentService;
use crate::domain::service::compaction_service::CompactionService;
use crate::domain::service::instruction_service::InstructionService;
use serde_json::json;
use tokio::sync::mpsc;
use uuid::Uuid;

// Current agent turn flow.
//
// start_turn:
//   - Validate the user message.
//   - Reject the turn unless the session is Idle.
//   - Mark the session Running.
//   - Save the user message.
//   - Build the initial compacted LLM context from the DB transcript.
//   - Delegate the rest of the turn to agent_loop.
//
// agent_loop:
//   - Call the LLM for one step.
//   - Save the assistant message and token usage before handling tools.
//   - Emit assistant text events for UI/API consumers.
//   - For allowed tool calls, execute the tool, save ToolCallOutput, then loop.
//   - For denied or errored tool calls, save an error ToolCallOutput, then loop.
//   - For tool calls that require approval, save awaiting_tool_approvals,
//     mark the session AwaitingApproval, emit a confirmation event, and stop.
//   - If the assistant message has no tool calls, mark the session Idle and finish.
//
// continue_after_tool_output:
//   - Before calling the LLM again, scan the DB transcript for already-saved
//     unresolved tool calls.
//   - If one exists, process it first. This preserves the order of multiple
//     tool calls from the same assistant message after an approval resumes.
//   - If none exists, rebuild context from the DB transcript and enter agent_loop.
//
// approve_approval:
//   - Load the awaiting approval from the DB.
//   - Restore the original ToolCall from the saved assistant message.
//   - Mark the session Running.
//   - Execute the approved tool and save ToolCallOutput.
//   - Record the approval audit row and delete awaiting_tool_approvals.
//   - Continue after the saved tool output, processing any remaining unresolved
//     tool calls before the next LLM step.
//
// deny_approval:
//   - Load the awaiting approval from the DB.
//   - Restore the original ToolCall from the saved assistant message.
//   - Mark the session Running.
//   - Save a denied ToolCallOutput without executing the tool.
//   - Record the denial audit row and delete awaiting_tool_approvals.
//   - Continue after the saved tool output, processing any remaining unresolved
//     tool calls before the next LLM step.

const MAX_LLM_STEPS: usize = 20;
const MAX_TOOL_OUTPUT_CHARS: usize = 50_000;

#[derive(Debug, Clone)]
pub enum Attachment {
    Image(InputImage),
    File(InputFile),
}

struct AwaitingToolCall {
    tool_call: ToolCall,
}

struct UnresolvedToolCall {
    assistant_message_id: Uuid,
    tool_call: ToolCall,
}

enum ToolCallStep {
    Continued,
    AwaitingApproval(AgentStartTurnOutput),
}

#[derive(Debug)]
pub struct AgentStartTurnOutput {
    pub events: Vec<AppEvent>,
}

pub struct AgentUsecase<L, S, M, T, A, W> {
    agent_service: AgentService<L>,
    instruction_service: InstructionService,
    compaction_service: CompactionService<L>,
    chat_session_repository: S,
    chat_message_repository: M,
    token_usage_repository: T,
    tool_approval_repository: A,
    awaiting_tool_approval_repository: W,
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
        agent_service: AgentService<L>,
        instruction_service: InstructionService,
        compaction_service: CompactionService<L>,
        repositories: AgentUsecaseRepositories<S, M, T, A, W>,
    ) -> Self {
        Self {
            agent_service,
            instruction_service,
            compaction_service,
            chat_session_repository: repositories.chat_session_repository,
            chat_message_repository: repositories.chat_message_repository,
            token_usage_repository: repositories.token_usage_repository,
            tool_approval_repository: repositories.tool_approval_repository,
            awaiting_tool_approval_repository: repositories.awaiting_tool_approval_repository,
        }
    }

    pub async fn submit_user_message(
        &self,
        session_id: Uuid,
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
            .append(session_id, user_message)
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

    pub async fn start_turn(
        &self,
        session_id: Uuid,
        user_message: ChatMessage,
        tx: mpsc::Sender<AppEvent>,
    ) -> Result<AgentStartTurnOutput, AgentUsecaseError> {
        let input_messages = self
            .load_compacted_input_messages(session_id, &user_message)
            .await?;

        let instruction = self.instruction_service.build_agent_instruction();

        self.agent_loop(session_id, instruction, input_messages, tx)
            .await
    }

    async fn stop_turn(&self, session_id: Uuid) -> Result<(), AgentUsecaseError> {
        let session = self
            .chat_session_repository
            .find_by_id(session_id)
            .await?
            .ok_or(AgentUsecaseError::SessionNotFound(session_id))?;
        let idle_status = session.complete_turn()?;

        self.chat_session_repository
            .update_status(session_id, idle_status)
            .await?;

        Ok(())
    }

    async fn load_compacted_input_messages(
        &self,
        session_id: Uuid,
        saved_user_message: &ChatMessage,
    ) -> Result<Vec<Message>, AgentUsecaseError> {
        let history_entries = self
            .chat_message_repository
            .list_for_session(session_id)
            .await?;

        let history = history_entries
            .into_iter()
            .map(|entry| {
                if entry.id == saved_user_message.id {
                    saved_user_message.message.clone()
                } else {
                    entry.message
                }
            })
            .collect::<Vec<_>>();

        let latest_usage = self
            .token_usage_repository
            .find_latest_for_session(session_id)
            .await?;

        self.compaction_service
            .compact_if_needed(history, latest_usage)
            .await
            .map_err(Into::into)
    }

    async fn save_llm_response(
        &self,
        session_id: Uuid,
        response: &LlmResponse,
    ) -> Result<ChatMessage, AgentUsecaseError> {
        let saved_message = self
            .chat_message_repository
            .append(session_id, response.message.clone())
            .await?;

        if !response.usage.is_empty() {
            self.token_usage_repository
                .record_for_message(saved_message.id, self.agent_service.model(), response.usage)
                .await?;
        }

        Ok(saved_message)
    }

    async fn save_tool_call_output(
        &self,
        session_id: Uuid,
        output: ToolCallOutput,
    ) -> Result<ToolCallOutput, AgentUsecaseError> {
        let output = output.truncate(MAX_TOOL_OUTPUT_CHARS);
        let message = Message::user_tool_call_outputs(vec![output.clone()])?;

        self.chat_message_repository
            .append(session_id, message)
            .await?;

        Ok(output)
    }

    async fn execute_and_save_tool_call(
        &self,
        session_id: Uuid,
        tool_call: &ToolCall,
        tx: &mpsc::Sender<AppEvent>,
    ) -> Result<ToolCallOutput, AgentUsecaseError> {
        let call_id = tool_call.call_id.clone();
        let tool_name = tool_call.name.clone();
        let arguments = tool_call.arguments.clone();

        let _ = tx
            .send(AppEvent::ToolCallStarted {
                session_id,
                call_id: call_id.clone(),
                tool_name: tool_name.clone(),
                arguments: arguments.clone(),
            })
            .await;

        let result = self
            .agent_service
            .tool_service()
            .execute(tool_call.clone())
            .await;

        let tool_call_output = match result {
            Ok(output) => output,
            Err(err) => tool_call_error_output(call_id.clone(), err.to_string()),
        };

        let tool_call_output = self
            .save_tool_call_output(session_id, tool_call_output)
            .await?;

        let output = tool_call_output.output.clone();
        let status = tool_call_output.status;

        let _ = tx
            .send(AppEvent::ToolCallFinished {
                session_id,
                call_id,
                tool_name,
                output,
                status,
            })
            .await;

        Ok(tool_call_output)
    }

    async fn load_compacted_session_messages(
        &self,
        session_id: Uuid,
    ) -> Result<Vec<Message>, AgentUsecaseError> {
        let history_entries = self
            .chat_message_repository
            .list_for_session(session_id)
            .await?;

        let history = history_entries
            .into_iter()
            .map(|entry| entry.message)
            .collect::<Vec<_>>();

        let latest_usage = self
            .token_usage_repository
            .find_latest_for_session(session_id)
            .await?;

        self.compaction_service
            .compact_if_needed(history, latest_usage)
            .await
            .map_err(Into::into)
    }

    async fn agent_loop(
        &self,
        session_id: Uuid,
        instruction: String,
        mut input_messages: Vec<Message>,
        tx: mpsc::Sender<AppEvent>,
    ) -> Result<AgentStartTurnOutput, AgentUsecaseError> {
        let mut events = Vec::new();
        let mut loop_safety = LoopSafety::new(MAX_LLM_STEPS);

        loop {
            if let Err(err) = loop_safety.start_llm_step() {
                self.stop_turn(session_id).await?;

                let event = AppEvent::AgentTurnFailed {
                    session_id,
                    reason: err.to_string(),
                };
                let _ = tx.send(event.clone()).await;
                events.push(event);

                return Err(AgentUsecaseError::Agent(AgentError::from(err)));
            }

            let _ = tx.send(AppEvent::LlmStarted { session_id }).await;

            let llm_response = self
                .agent_service
                .llm_step(instruction.clone(), input_messages.clone())
                .await?;

            let _ = tx.send(AppEvent::LlmFinished { session_id }).await;

            let saved_agent_message = self.save_llm_response(session_id, &llm_response).await?;

            for event in
                assistant_text_events(session_id, saved_agent_message.id, &llm_response.message)
            {
                let _ = tx.send(event.clone()).await;
                events.push(event);
            }

            // Token usage events
            if !llm_response.usage.is_empty() {
                let event = AppEvent::LlmUsageRecorded {
                    session_id,
                    message_id: saved_agent_message.id,
                    usage: llm_response.usage,
                };

                let _ = tx.send(event.clone()).await;
                events.push(event);
            }

            let tool_calls = llm_response.message.tool_calls();

            if tool_calls.is_empty() {
                let session = self
                    .chat_session_repository
                    .find_by_id(session_id)
                    .await?
                    .ok_or(AgentUsecaseError::SessionNotFound(session_id))?;
                let idle_status = session.complete_turn()?;

                self.chat_session_repository
                    .update_status(session_id, idle_status)
                    .await?;

                let event = AppEvent::AgentTurnCompleted { session_id };
                let _ = tx.send(event.clone()).await;
                events.push(event);

                return Ok(AgentStartTurnOutput { events });
            }

            let mut saved_tool_output = false;

            for tool_call in tool_calls {
                match self
                    .process_tool_call(
                        session_id,
                        saved_agent_message.id,
                        tool_call,
                        &mut events,
                        &mut loop_safety,
                        &tx,
                    )
                    .await?
                {
                    ToolCallStep::Continued => {
                        saved_tool_output = true;
                    }
                    ToolCallStep::AwaitingApproval(output) => {
                        return Ok(output);
                    }
                }
            }

            if saved_tool_output {
                input_messages = self.load_compacted_session_messages(session_id).await?;
                continue;
            }
        }
    }

    async fn load_awaiting_tool_call(
        &self,
        session_id: Uuid,
    ) -> Result<AwaitingToolCall, AgentUsecaseError> {
        let awaiting = self
            .awaiting_tool_approval_repository
            .find_by_session_id(session_id)
            .await?
            .ok_or(AgentUsecaseError::ChatSession(
                ChatSessionError::ApprovalNotPending { session_id },
            ))?;

        let messages = self
            .chat_message_repository
            .list_for_session(session_id)
            .await?;

        let assistant_message = messages
            .into_iter()
            .find(|entry| entry.id == awaiting.assistant_message_id)
            .ok_or_else(|| {
                AgentUsecaseError::ApprovalState(format!(
                    "awaiting approval assistant message not found: {}",
                    awaiting.assistant_message_id
                ))
            })?;

        let tool_call = assistant_message
            .message
            .find_tool_call(&awaiting.tool_call_id)
            .ok_or_else(|| {
                AgentUsecaseError::ApprovalState(format!(
                    "awaiting approval tool call not found: {}",
                    awaiting.tool_call_id
                ))
            })?;

        Ok(AwaitingToolCall { tool_call })
    }

    async fn save_denied_tool_call_output(
        &self,
        session_id: Uuid,
        tool_call: &ToolCall,
    ) -> Result<ToolCallOutput, AgentUsecaseError> {
        let output = tool_call_error_output(
            tool_call.call_id.clone(),
            "tool execution was denied by user",
        );

        self.save_tool_call_output(session_id, output).await
    }

    async fn record_tool_approval_from_tool_call(
        &self,
        session_id: Uuid,
        tool_call: &ToolCall,
        decision: ToolApprovalResponse,
    ) -> Result<(), AgentUsecaseError> {
        self.tool_approval_repository
            .record(ToolApproval {
                session_id,
                tool_call_id: tool_call.call_id.clone(),
                tool_name: tool_call.name.clone(),
                arguments: tool_call.arguments.clone(),
                decision,
            })
            .await?;

        Ok(())
    }

    pub async fn resolve_awaiting_approval(
        &self,
        session_id: Uuid,
        decision: ToolApprovalResponse,
        tx: mpsc::Sender<AppEvent>,
    ) -> Result<AgentStartTurnOutput, AgentUsecaseError> {
        let session = self
            .chat_session_repository
            .find_by_id(session_id)
            .await?
            .ok_or(AgentUsecaseError::SessionNotFound(session_id))?;
        let next_status = session.resolve_approval()?;

        let awaiting = self.load_awaiting_tool_call(session_id).await?;
        let tool_call = awaiting.tool_call;

        self.chat_session_repository
            .update_status(session_id, next_status)
            .await?;

        let resolved = AppEvent::ToolCallApprovalResolved {
            session_id,
            call_id: tool_call.call_id.clone(),
            tool_name: tool_call.name.clone(),
            decision,
        };
        let _ = tx.send(resolved).await;

        match decision {
            ToolApprovalResponse::Approved => {
                self.execute_and_save_tool_call(session_id, &tool_call, &tx)
                    .await?;
            }
            ToolApprovalResponse::Denied => {
                let output = self
                    .save_denied_tool_call_output(session_id, &tool_call)
                    .await?;

                let _ = tx
                    .send(AppEvent::ToolCallFinished {
                        session_id,
                        call_id: tool_call.call_id.clone(),
                        tool_name: tool_call.name.clone(),
                        output: output.output,
                        status: output.status,
                    })
                    .await;
            }
        }

        self.record_tool_approval_from_tool_call(session_id, &tool_call, decision)
            .await?;

        self.awaiting_tool_approval_repository
            .delete_by_session_id(session_id)
            .await?;

        self.continue_after_tool_output(session_id, tx).await
    }

    async fn next_unresolved_tool_call(
        &self,
        session_id: Uuid,
    ) -> Result<Option<UnresolvedToolCall>, AgentUsecaseError> {
        let messages = self
            .chat_message_repository
            .list_for_session(session_id)
            .await?;

        let mut resolved_call_ids = std::collections::HashSet::new();

        for entry in &messages {
            for content in &entry.message.content {
                if let MessageContent::ToolCallOutput(output) = content {
                    resolved_call_ids.insert(output.call_id.clone());
                }
            }
        }

        for entry in messages {
            if entry.message.role != Role::Assistant {
                continue;
            }

            for call in entry.message.tool_calls() {
                if !resolved_call_ids.contains(&call.call_id) {
                    return Ok(Some(UnresolvedToolCall {
                        assistant_message_id: entry.id,
                        tool_call: call,
                    }));
                }
            }
        }

        Ok(None)
    }

    async fn process_tool_call(
        &self,
        session_id: Uuid,
        assistant_message_id: Uuid,
        tool_call: ToolCall,
        events: &mut Vec<AppEvent>,
        loop_safety: &mut LoopSafety,
        tx: &mpsc::Sender<AppEvent>,
    ) -> Result<ToolCallStep, AgentUsecaseError> {
        match self
            .agent_service
            .tool_service()
            .decide_execution(&tool_call)
            .await
        {
            Ok(ToolExecutionDecision::Allow) => {
                let output = self
                    .execute_and_save_tool_call(session_id, &tool_call, tx)
                    .await?;

                if let Err(err) = loop_safety.record_tool_call_output(&tool_call, &output) {
                    self.stop_turn(session_id).await?;

                    let event = AppEvent::AgentTurnFailed {
                        session_id,
                        reason: err.to_string(),
                    };
                    let _ = tx.send(event.clone()).await;
                    events.push(event);

                    return Err(AgentUsecaseError::Agent(AgentError::from(err)));
                }

                Ok(ToolCallStep::Continued)
            }
            Ok(ToolExecutionDecision::Ask) => {
                let policy = self
                    .agent_service
                    .tool_service()
                    .check_execution_policy(&tool_call)?;
                let session = self
                    .chat_session_repository
                    .find_by_id(session_id)
                    .await?
                    .ok_or(AgentUsecaseError::SessionNotFound(session_id))?;
                let next_status = session.await_approval()?;

                self.awaiting_tool_approval_repository
                    .save(AwaitingToolApproval {
                        session_id,
                        assistant_message_id,
                        tool_call_id: tool_call.call_id.clone(),
                    })
                    .await?;

                let event = AppEvent::ToolCallApprovalRequested {
                    session_id,
                    call_id: tool_call.call_id,
                    tool_name: tool_call.name,
                    arguments: tool_call.arguments,
                    policy,
                };

                let _ = tx.send(event.clone()).await;
                events.push(event);

                self.chat_session_repository
                    .update_status(session_id, next_status)
                    .await?;

                Ok(ToolCallStep::AwaitingApproval(AgentStartTurnOutput {
                    events: std::mem::take(events),
                }))
            }
            Ok(ToolExecutionDecision::Deny) => {
                let output = tool_call_error_output(
                    tool_call.call_id.clone(),
                    "tool execution was blocked by execution rule",
                );

                let output = self.save_tool_call_output(session_id, output).await?;

                let _ = tx
                    .send(AppEvent::ToolCallFinished {
                        session_id,
                        call_id: tool_call.call_id.clone(),
                        tool_name: tool_call.name.clone(),
                        output: output.output.clone(),
                        status: output.status,
                    })
                    .await;

                if let Err(err) = loop_safety.record_tool_call_output(&tool_call, &output) {
                    self.stop_turn(session_id).await?;

                    let event = AppEvent::AgentTurnFailed {
                        session_id,
                        reason: err.to_string(),
                    };
                    let _ = tx.send(event.clone()).await;
                    events.push(event);

                    return Err(AgentUsecaseError::Agent(AgentError::from(err)));
                }

                Ok(ToolCallStep::Continued)
            }
            Err(err) => {
                let output = tool_call_error_output(tool_call.call_id.clone(), err.to_string());

                let output = self.save_tool_call_output(session_id, output).await?;

                let _ = tx
                    .send(AppEvent::ToolCallFinished {
                        session_id,
                        call_id: tool_call.call_id.clone(),
                        tool_name: tool_call.name.clone(),
                        output: output.output.clone(),
                        status: output.status,
                    })
                    .await;

                if let Err(err) = loop_safety.record_tool_call_output(&tool_call, &output) {
                    self.stop_turn(session_id).await?;

                    let event = AppEvent::AgentTurnFailed {
                        session_id,
                        reason: err.to_string(),
                    };
                    let _ = tx.send(event.clone()).await;
                    events.push(event);

                    return Err(AgentUsecaseError::Agent(AgentError::from(err)));
                }

                Ok(ToolCallStep::Continued)
            }
        }
    }

    async fn continue_after_tool_output(
        &self,
        session_id: Uuid,
        tx: mpsc::Sender<AppEvent>,
    ) -> Result<AgentStartTurnOutput, AgentUsecaseError> {
        let mut events = Vec::new();
        let mut loop_safety = LoopSafety::new(MAX_LLM_STEPS);

        loop {
            if let Some(unresolved) = self.next_unresolved_tool_call(session_id).await? {
                match self
                    .process_tool_call(
                        session_id,
                        unresolved.assistant_message_id,
                        unresolved.tool_call,
                        &mut events,
                        &mut loop_safety,
                        &tx,
                    )
                    .await?
                {
                    ToolCallStep::Continued => continue,
                    ToolCallStep::AwaitingApproval(output) => return Ok(output),
                }
            }

            let input_messages = self.load_compacted_session_messages(session_id).await?;
            let instruction = self.instruction_service.build_agent_instruction();

            let output = self
                .agent_loop(session_id, instruction, input_messages, tx)
                .await?;

            let mut all_events = events;
            all_events.extend(output.events);

            return Ok(AgentStartTurnOutput { events: all_events });
        }
    }

    pub async fn list_awaiting_approvals(
        &self,
    ) -> Result<Vec<AwaitingToolApproval>, AgentUsecaseError> {
        self.awaiting_tool_approval_repository
            .list_all()
            .await
            .map_err(Into::into)
    }
}

fn assistant_text_events(session_id: Uuid, message_id: Uuid, message: &Message) -> Vec<AppEvent> {
    message
        .output_texts()
        .into_iter()
        .map(|content| AppEvent::AssistantMessageCreated {
            session_id,
            message_id,
            content,
        })
        .collect()
}

fn tool_call_error_output(
    call_id: impl Into<String>,
    message: impl Into<String>,
) -> ToolCallOutput {
    ToolCallOutput::error(
        call_id,
        json!({
            "message": message.into(),
        }),
    )
}
