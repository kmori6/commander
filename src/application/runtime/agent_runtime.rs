use serde_json::{Value, json};
use std::sync::Arc;
use uuid::Uuid;

use crate::application::error::agent_runtime_error::AgentRuntimeError;
use crate::domain::model::event::Event;
use crate::domain::model::message::{Message, MessageContent, Role};
use crate::domain::model::task::{Task, TaskStatus};
use crate::domain::model::task_result::TaskResultStatus;
use crate::domain::model::tool_call::{
    ToolApprovalStatus, ToolCall, ToolCallOutput, ToolPermissionMode,
};
use crate::domain::port::llm_provider::{LlmMessage, LlmProvider, LlmRequest};
use crate::domain::repository::event_repository::EventRepository;
use crate::domain::repository::message_repository::MessageRepository;
use crate::domain::repository::task_repository::TaskRepository;
use crate::domain::repository::task_result_repository::TaskResultRepository;
use crate::domain::repository::token_usage_repository::{CreateTokenUsage, TokenUsageRepository};
use crate::domain::repository::tool_approval_repository::ToolApprovalRepository;
use crate::domain::repository::tool_permission_repository::ToolPermissionRepository;
use crate::domain::service::event_service::EventService;
use crate::domain::service::tool_executor::ToolExecutor;

const MAX_LLM_STEPS: usize = 20;

enum ToolCallRunOutcome {
    Continue,
    AwaitingApproval,
}

#[derive(Clone)]
pub struct AgentRuntime<L, T, M, R, E, U, P, A> {
    llm_provider: L,
    tool_executor: Arc<ToolExecutor>,
    task_repository: T,
    message_repository: M,
    task_result_repository: R,
    event_repository: E,
    token_usage_repository: U,
    tool_permission_repository: P,
    tool_approval_repository: A,
    event_service: Arc<EventService>,
    model: String,
}

impl<L, T, M, R, E, U, P, A> AgentRuntime<L, T, M, R, E, U, P, A>
where
    L: LlmProvider,
    T: TaskRepository,
    M: MessageRepository,
    R: TaskResultRepository,
    E: EventRepository,
    U: TokenUsageRepository,
    P: ToolPermissionRepository,
    A: ToolApprovalRepository,
{
    pub fn new(
        llm_provider: L,
        tool_executor: Arc<ToolExecutor>,
        task_repository: T,
        message_repository: M,
        task_result_repository: R,
        event_repository: E,
        token_usage_repository: U,
        event_service: Arc<EventService>,
        tool_permission_repository: P,
        tool_approval_repository: A,
        model: String,
    ) -> Self {
        Self {
            llm_provider,
            tool_executor,
            task_repository,
            message_repository,
            task_result_repository,
            event_repository,
            token_usage_repository,
            event_service,
            tool_permission_repository,
            tool_approval_repository,
            model,
        }
    }

    pub fn llm_provider(&self) -> &L {
        &self.llm_provider
    }

    // agent runtime -> event service -> event handler -> sse event
    async fn emit(
        &self,
        task_id: Uuid,
        event_type: &str,
        payload: Value,
    ) -> Result<Event, AgentRuntimeError> {
        let event = self
            .event_repository
            .save(task_id, event_type, payload)
            .await?;

        self.event_service.publish(event.clone());

        Ok(event)
    }

    pub async fn run(
        &self,
        task_id: Uuid,
        user_contents: Option<Vec<MessageContent>>,
    ) -> Result<(), AgentRuntimeError> {
        match self.execute(task_id, true, user_contents).await {
            Ok(()) => Ok(()),
            Err(err) => {
                if let Err(fail_err) = self.fail_task(task_id, &err).await {
                    log::warn!("failed to mark task {task_id} as failed: {fail_err}");
                }
                Err(err)
            }
        }
    }

    async fn execute(
        &self,
        task_id: Uuid,
        emit_started: bool,
        user_contents: Option<Vec<MessageContent>>,
    ) -> Result<(), AgentRuntimeError> {
        let task = self
            .task_repository
            .find_by_id(task_id)
            .await?
            .ok_or(AgentRuntimeError::TaskNotFound)?;

        if emit_started {
            self.emit(task_id, "task_started", json!({})).await?;
        }

        self.task_repository
            .update_status(task_id, TaskStatus::Running)
            .await?;

        for step in 0..MAX_LLM_STEPS {
            let messages = self
                .build_llm_messages(task.session_id, &task.request, user_contents.as_ref())
                .await?;

            self.emit(
                task_id,
                "llm_started",
                json!({
                    "model": self.model,
                    "step": step + 1,
                }),
            )
            .await?;

            let response = match self
                .llm_provider
                .respond(
                    LlmRequest::new(self.model.clone(), messages)
                        .with_tools(self.tool_executor.specs()),
                )
                .await
            {
                Ok(response) => response,
                Err(err) => {
                    self.emit(
                        task_id,
                        "llm_failed",
                        json!({
                            "model": self.model,
                            "step": step + 1,
                            "error": err.to_string(),
                        }),
                    )
                    .await?;

                    return Err(err.into());
                }
            };

            self.emit(
                task_id,
                "llm_finished",
                json!({
                    "model": self.model,
                    "step": step + 1,
                    "input_tokens": response.usage.input_tokens,
                    "output_tokens": response.usage.output_tokens,
                    "cache_read_tokens": response.usage.cache_read_tokens,
                    "cache_write_tokens": response.usage.cache_write_tokens,
                }),
            )
            .await?;

            let assistant_message = self
                .message_repository
                .save(
                    task.session_id,
                    Role::Assistant,
                    response.message.contents.clone(),
                )
                .await?;

            self.token_usage_repository
                .save(CreateTokenUsage {
                    task_id,
                    message_id: Some(assistant_message.id),
                    model: self.model.clone(),
                    input_tokens: response.usage.input_tokens,
                    output_tokens: response.usage.output_tokens,
                    cache_read_tokens: response.usage.cache_read_tokens,
                    cache_write_tokens: response.usage.cache_write_tokens,
                })
                .await?;

            let tool_calls = response
                .message
                .contents
                .iter()
                .filter_map(ToolCall::from_message_content)
                .collect::<Vec<_>>();

            if tool_calls.is_empty() {
                self.complete_task(task_id, response.output_text("\n"))
                    .await?;
                return Ok(());
            }

            match self
                .run_tool_calls(task_id, task.session_id, assistant_message.id, tool_calls)
                .await?
            {
                ToolCallRunOutcome::Continue => {}
                ToolCallRunOutcome::AwaitingApproval => return Ok(()),
            }
        }

        Err(AgentRuntimeError::Unsupported(format!(
            "maximum LLM steps exceeded: {MAX_LLM_STEPS}"
        )))
    }

    pub async fn resume(&self, approval_id: Uuid) -> Result<(), AgentRuntimeError> {
        let task_id = self.apply_approval(approval_id).await?;

        match self.execute(task_id, false, None).await {
            Ok(()) => Ok(()),
            Err(err) => {
                if let Err(fail_err) = self.fail_task(task_id, &err).await {
                    log::warn!("failed to mark task {task_id} as failed: {fail_err}");
                }
                Err(err)
            }
        }
    }

    async fn apply_approval(&self, approval_id: Uuid) -> Result<Uuid, AgentRuntimeError> {
        let approval = self
            .tool_approval_repository
            .find_by_id(approval_id)
            .await?
            .ok_or(AgentRuntimeError::ToolApprovalNotFound)?;

        if approval.status == ToolApprovalStatus::Pending {
            return Err(AgentRuntimeError::ToolApprovalPending(approval_id));
        }

        let message = self
            .message_repository
            .find_by_id(approval.message_id)
            .await?
            .ok_or(AgentRuntimeError::MessageNotFound(approval.message_id))?;

        let task = self
            .task_repository
            .find_by_session_id(message.session_id)
            .await?
            .ok_or(AgentRuntimeError::TaskNotFound)?;

        let call = message
            .contents
            .iter()
            .filter_map(ToolCall::from_message_content)
            .find(|call| call.call_id == approval.call_id)
            .ok_or_else(|| AgentRuntimeError::ToolCallNotFound(approval.call_id.clone()))?;

        let output = match approval.status {
            ToolApprovalStatus::Approved => match self.tool_executor.execute(call.clone()).await {
                Ok(output) => output,
                Err(err) => ToolCallOutput::error(call.call_id.clone(), err.to_string()),
            },
            ToolApprovalStatus::Rejected => ToolCallOutput::error(
                call.call_id.clone(),
                format!("tool execution rejected: {}", call.tool_name),
            ),
            ToolApprovalStatus::Pending => unreachable!(),
        };

        self.emit(
            task.id,
            "tool_approval_resolved",
            json!({
                "approval_id": approval.id.to_string(),
                "call_id": call.call_id,
                "tool_name": call.tool_name,
                "status": approval.status.as_str(),
            }),
        )
        .await?;

        self.emit(
            task.id,
            "tool_call_finished",
            json!({
                "call_id": output.call_id,
                "output": output.output,
                "status": output.status.as_str(),
            }),
        )
        .await?;

        self.message_repository
            .save(
                message.session_id,
                Role::User,
                vec![output.into_message_content()],
            )
            .await?;

        Ok(task.id)
    }

    async fn build_llm_messages(
        &self,
        session_id: Uuid,
        request: &str,
        user_contents: Option<&Vec<MessageContent>>,
    ) -> Result<Vec<LlmMessage>, AgentRuntimeError> {
        let mut messages = Vec::new();

        messages.push(LlmMessage::system_text(
        "You are Commander, an autonomous task execution agent. Complete the given task clearly and concisely.",
    ));

        match user_contents {
            Some(contents) => {
                messages.push(LlmMessage::new(Role::User, contents.clone()));
            }
            None => {
                messages.push(LlmMessage::user_text(request.to_string()));
            }
        }

        // load previous messages from the task session (does not include the user message)
        let session_messages = self.message_repository.list_for_session(session_id).await?;
        messages.extend(session_messages.into_iter().map(to_llm_message));

        Ok(messages)
    }

    async fn run_tool_calls(
        &self,
        task_id: Uuid,
        session_id: Uuid,
        assistant_message_id: Uuid,
        tool_calls: Vec<ToolCall>,
    ) -> Result<ToolCallRunOutcome, AgentRuntimeError> {
        let mut outputs = Vec::new();

        for call in tool_calls {
            self.emit(
                task_id,
                "tool_call_started",
                json!({
                    "call_id": call.call_id,
                    "tool_name": call.tool_name,
                    "arguments": call.arguments,
                }),
            )
            .await?;

            let mode = self.resolve_tool_permission(&call.tool_name).await?;

            self.emit(
                task_id,
                "tool_call_permission_resolved",
                json!({
                    "call_id": call.call_id,
                    "tool_name": call.tool_name,
                    "mode": mode.as_str(),
                }),
            )
            .await?;

            let output = match mode {
                ToolPermissionMode::Allow => match self.tool_executor.execute(call.clone()).await {
                    Ok(output) => output,
                    Err(err) => ToolCallOutput::error(call.call_id.clone(), err.to_string()),
                },
                ToolPermissionMode::Deny => ToolCallOutput::error(
                    call.call_id.clone(),
                    format!("tool execution denied: {}", call.tool_name),
                ),
                ToolPermissionMode::Ask => {
                    if !outputs.is_empty() {
                        self.message_repository
                            .save(session_id, Role::User, outputs)
                            .await?;
                    }

                    let approval = self
                        .tool_approval_repository
                        .create_pending(assistant_message_id, &call.call_id)
                        .await?;

                    self.task_repository
                        .update_status(task_id, TaskStatus::AwaitingApproval)
                        .await?;

                    self.emit(
                        task_id,
                        "tool_approval_requested",
                        json!({
                            "approval_id": approval.id.to_string(),
                            "message_id": assistant_message_id.to_string(),
                            "call_id": call.call_id,
                            "tool_name": call.tool_name,
                        }),
                    )
                    .await?;

                    return Ok(ToolCallRunOutcome::AwaitingApproval);
                }
            };

            self.emit(
                task_id,
                "tool_call_finished",
                json!({
                    "call_id": output.call_id,
                    "output": output.output,
                    "status": output.status.as_str(),
                }),
            )
            .await?;

            outputs.push(output.into_message_content());
        }

        self.message_repository
            .save(session_id, Role::User, outputs)
            .await?;

        Ok(ToolCallRunOutcome::Continue)
    }

    async fn complete_task(&self, task_id: Uuid, output: String) -> Result<(), AgentRuntimeError> {
        let task = self
            .task_repository
            .find_by_id(task_id)
            .await?
            .ok_or(AgentRuntimeError::TaskNotFound)?;

        let result = self
            .task_result_repository
            .save(task_id, TaskResultStatus::Success, output.clone())
            .await?;

        self.save_chat_assistant_message(&task, &output).await?;

        self.emit(
            task_id,
            "task_result_created",
            json!({
                "result_id": result.id.to_string(),
                "status": result.status.as_str(),
            }),
        )
        .await?;

        self.task_repository
            .update_status(task_id, TaskStatus::Completed)
            .await?;

        self.emit(task_id, "task_completed", json!({})).await?;

        Ok(())
    }

    async fn fail_task(
        &self,
        task_id: Uuid,
        err: &AgentRuntimeError,
    ) -> Result<(), AgentRuntimeError> {
        let output = err.to_string();

        let result = self
            .task_result_repository
            .save(task_id, TaskResultStatus::Failure, output.clone())
            .await?;

        self.emit(
            task_id,
            "task_result_created",
            json!({
                "result_id": result.id.to_string(),
                "status": result.status.as_str(),
            }),
        )
        .await?;

        self.task_repository
            .update_status(task_id, TaskStatus::Failed)
            .await?;

        self.emit(
            task_id,
            "task_failed",
            json!({
                "error": output,
            }),
        )
        .await?;

        Ok(())
    }

    async fn resolve_tool_permission(
        &self,
        tool_name: &str,
    ) -> Result<ToolPermissionMode, AgentRuntimeError> {
        if let Some(permission) = self
            .tool_permission_repository
            .find_by_tool_name(tool_name)
            .await?
        {
            return Ok(permission.mode);
        }

        Ok(self
            .tool_executor
            .default_permission(tool_name)
            .unwrap_or(ToolPermissionMode::Deny))
    }

    async fn save_chat_assistant_message(
        &self,
        task: &Task,
        output: &str,
    ) -> Result<(), AgentRuntimeError> {
        let Some(source_message_id) = task.source_message_id else {
            return Ok(());
        };

        let Some(source_message) = self
            .message_repository
            .find_by_id(source_message_id)
            .await?
        else {
            return Ok(());
        };

        self.message_repository
            .save(
                source_message.session_id,
                Role::Assistant,
                vec![MessageContent::output_text(output.to_string())],
            )
            .await?;

        Ok(())
    }
}

fn to_llm_message(message: Message) -> LlmMessage {
    LlmMessage::new(message.role, message.contents)
}
