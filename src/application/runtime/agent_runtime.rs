use crate::application::error::agent_runtime_error::AgentRuntimeError;
use crate::application::runtime::subagent_call::{
    SubagentCall, SubagentResult, SubagentResultStatus,
};
use crate::application::service::compaction_service::{CompactionConfig, CompactionService};
use crate::application::service::event_service::EventService;
use crate::application::service::instruction_service::InstructionService;
use crate::application::service::tool_executor::ToolExecutor;
use crate::application::service::tool_permitter::ToolPermitter;
use crate::domain::model::message::{Message, MessageContent, Role};
use crate::domain::model::subagent::Subagent;
use crate::domain::model::task::{Task, TaskStatus};
use crate::domain::model::tool_call::{
    ToolApproval, ToolApprovalStatus, ToolCall, ToolCallOutput, ToolPermissionMode, ToolSpec,
};
use crate::domain::port::llm_provider::{LlmMessage, LlmProvider, LlmRequest};
use crate::domain::repository::message_repository::MessageRepository;
use crate::domain::repository::subagent_repository::SubagentRepository;
use crate::domain::repository::task_repository::TaskRepository;
use crate::domain::repository::tool_approval_repository::ToolApprovalRepository;
use crate::domain::repository::tool_permission_repository::ToolPermissionRepository;
use async_recursion::async_recursion;
use serde_json::{Value, json};
use std::sync::Arc;
use uuid::Uuid;

const MAX_LLM_STEPS: usize = 30;

enum LoopState {
    Durable { task: Task },
    Ephemeral { messages: Vec<LlmMessage> },
}

enum LoopOutcome {
    Completed(String),
    AwaitingApproval,
    Stopped,
}

enum ToolRun {
    Continue,
    AwaitingApproval,
    Stopped,
}

pub struct AgentRuntime<L, T, M, P, A> {
    llm_provider: L,
    tool_executor: Arc<ToolExecutor>,
    tool_permitter: Arc<ToolPermitter<P, A>>,
    task_repository: T,
    message_repository: M,
    subagent_repository: Arc<dyn SubagentRepository>,
    event_service: Arc<EventService>,
    instruction_service: Arc<InstructionService>,
}

impl<L, T, M, P, A> AgentRuntime<L, T, M, P, A>
where
    L: LlmProvider,
    T: TaskRepository,
    M: MessageRepository,
    P: ToolPermissionRepository,
    A: ToolApprovalRepository,
{
    pub fn new(
        llm_provider: L,
        tool_executor: Arc<ToolExecutor>,
        tool_permitter: Arc<ToolPermitter<P, A>>,
        task_repository: T,
        message_repository: M,
        subagent_repository: Arc<dyn SubagentRepository>,
        event_service: Arc<EventService>,
        instruction_service: Arc<InstructionService>,
    ) -> Self {
        Self {
            llm_provider,
            tool_executor,
            tool_permitter,
            task_repository,
            message_repository,
            subagent_repository,
            event_service,
            instruction_service,
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
    ) -> Result<(), AgentRuntimeError> {
        self.event_service.publish(task_id, event_type, payload);
        Ok(())
    }

    async fn build_llm_messages(
        &self,
        task: &Task,
        subagent: Option<&Subagent>,
        model: &str,
    ) -> Result<Vec<LlmMessage>, AgentRuntimeError> {
        let mut instruction = self.instruction_service.build_agent_instruction();

        if let Some(subagent) = subagent {
            instruction.push_str("\n\n# Child Agent Profile\n");
            instruction.push_str(&subagent.instruction);
        }

        let mut messages = vec![LlmMessage::system_text(instruction)];
        let context_messages = self.context_messages(task).await?;
        messages.extend(self.compact_messages(model, context_messages).await?);

        Ok(messages)
    }

    async fn context_messages(&self, task: &Task) -> Result<Vec<Message>, AgentRuntimeError> {
        if let Some(session_id) = task.session_id() {
            return self
                .message_repository
                .list_for_session(session_id, Some(task.id))
                .await
                .map_err(Into::into);
        }

        self.message_repository
            .list_for_task(task.id)
            .await
            .map_err(Into::into)
    }

    async fn compact_messages(
        &self,
        model: &str,
        messages: Vec<Message>,
    ) -> Result<Vec<LlmMessage>, AgentRuntimeError> {
        let context_window = self
            .llm_provider
            .context_window(model)
            .await
            .try_into()
            .map_err(|_| AgentRuntimeError::Unsupported(format!("unsupported model: {model}")))?;

        let service = CompactionService::new(CompactionConfig::for_window(context_window));

        let Some(result) = service
            .compact(&self.llm_provider, model, messages.clone())
            .await?
        else {
            return Ok(messages.into_iter().map(message_to_llm).collect());
        };

        let retained = if let Some(index) = messages
            .iter()
            .position(|message| message.id == result.until)
        {
            messages.into_iter().skip(index + 1).collect::<Vec<_>>()
        } else {
            Vec::new()
        };

        let mut compacted = vec![LlmMessage::system_text(format!(
            "# Compacted Conversation\n\n{}",
            result.summary
        ))];

        compacted.extend(retained.into_iter().map(message_to_llm));
        Ok(compacted)
    }

    async fn append_tool_call_outputs(
        &self,
        task_id: Uuid,
        state: &mut LoopState,
        outputs: &mut Vec<MessageContent>,
    ) -> Result<(), AgentRuntimeError> {
        if outputs.is_empty() {
            return Ok(());
        }

        let contents = std::mem::take(outputs);

        match state {
            LoopState::Durable { .. } => {
                self.message_repository
                    .save(task_id, Role::User, contents)
                    .await?;
            }
            LoopState::Ephemeral { messages } => {
                messages.push(LlmMessage::new(Role::User, contents));
            }
        }

        Ok(())
    }

    // root agent entry
    pub async fn run(&self, task_id: Uuid) -> Result<(), AgentRuntimeError> {
        let result = async {
            let task = self
                .task_repository
                .find_by_id(task_id)
                .await?
                .ok_or(AgentRuntimeError::TaskNotFound)?;

            let task = if task.status == TaskStatus::Running {
                task
            } else {
                self.task_repository.start(task_id).await?
            };

            self.emit(task_id, "task_started", json!({})).await?;

            if self.is_cancelled(task_id).await? {
                return Ok(());
            }

            self.apply_approvals(task_id).await?;

            let mut state = LoopState::Durable { task };

            match self.run_loop(task_id, None, &mut state).await? {
                LoopOutcome::Completed(output) => self.complete(task_id, output).await?,
                LoopOutcome::AwaitingApproval | LoopOutcome::Stopped => {}
            }

            Ok(())
        }
        .await;

        match result {
            Ok(()) => Ok(()),
            Err(err) => {
                if let Err(fail_err) = self.fail(task_id, &err).await {
                    log::warn!("failed to mark task {task_id} as failed: {fail_err}");
                }
                Err(err)
            }
        }
    }

    // agent loop
    #[async_recursion]
    async fn run_loop(
        &self,
        task_id: Uuid,
        subagent: Option<Subagent>,
        state: &mut LoopState,
    ) -> Result<LoopOutcome, AgentRuntimeError> {
        for step in 0..MAX_LLM_STEPS {
            if self.is_cancelled(task_id).await? {
                return Ok(LoopOutcome::Stopped);
            }

            let model = self.llm_provider.current_model_id().await?;
            let current_subagent = subagent.as_ref();

            let messages = match state {
                LoopState::Durable { task } => {
                    self.build_llm_messages(task, current_subagent, &model)
                        .await?
                }
                LoopState::Ephemeral { messages } => messages.clone(),
            };

            self.emit(
                task_id,
                "llm_started",
                json!({
                    "model": model.clone(),
                    "step": step + 1,
                }),
            )
            .await?;

            let response = match self
                .llm_provider
                .respond(
                    LlmRequest::new(model.clone(), messages)
                        .with_tools(self.tool_specs(current_subagent).await?),
                )
                .await
            {
                Ok(response) => response,
                Err(err) => {
                    self.emit(
                        task_id,
                        "llm_failed",
                        json!({
                            "model": model.clone(),
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
                    "model": model.clone(),
                    "step": step + 1,
                    "input_tokens": response.usage.input_tokens,
                    "output_tokens": response.usage.output_tokens,
                    "cache_read_tokens": response.usage.cache_read_tokens,
                    "cache_write_tokens": response.usage.cache_write_tokens,
                }),
            )
            .await?;

            // save assistant message if root agent
            let assistant_message_id = match state {
                LoopState::Durable { .. } => {
                    let assistant_message = self
                        .message_repository
                        .save_response(
                            task_id,
                            response.message.contents.clone(),
                            &model,
                            response.usage,
                        )
                        .await?;

                    Some(assistant_message.id)
                }
                LoopState::Ephemeral { messages } => {
                    messages.push(response.message.clone());
                    None
                }
            };

            // tool calls -> tool call outputs
            let tool_calls = response
                .message
                .contents
                .iter()
                .filter_map(ToolCall::from_message_content)
                .collect::<Vec<_>>();

            if tool_calls.is_empty() {
                if self.is_cancelled(task_id).await? {
                    return Ok(LoopOutcome::Stopped);
                }

                return Ok(LoopOutcome::Completed(response.output_text("\n")));
            }

            match self
                .run_tools(
                    task_id,
                    assistant_message_id,
                    tool_calls,
                    current_subagent,
                    state,
                )
                .await?
            {
                ToolRun::Continue => {}
                ToolRun::AwaitingApproval => return Ok(LoopOutcome::AwaitingApproval),
                ToolRun::Stopped => return Ok(LoopOutcome::Stopped),
            }
        }

        Err(AgentRuntimeError::Unsupported(format!(
            "maximum LLM steps exceeded: {MAX_LLM_STEPS}"
        )))
    }

    async fn run_tools(
        &self,
        task_id: Uuid,
        assistant_message_id: Option<Uuid>,
        tool_calls: Vec<ToolCall>,
        subagent: Option<&Subagent>,
        state: &mut LoopState,
    ) -> Result<ToolRun, AgentRuntimeError> {
        let mut outputs = Vec::new();

        for call in tool_calls {
            if self.is_cancelled(task_id).await? {
                self.append_tool_call_outputs(task_id, state, &mut outputs)
                    .await?;
                return Ok(ToolRun::Stopped);
            }

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

            // Subagent is a runtime call, so it is permitted only for the root agent.
            let mode = if call.tool_name == SubagentCall::TOOL_NAME {
                if subagent.is_none() {
                    ToolPermissionMode::Allow
                } else {
                    ToolPermissionMode::Deny
                }
            } else {
                self.tool_permission(&call.tool_name, subagent).await?
            };

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
                ToolPermissionMode::Deny => ToolCallOutput::error(
                    call.call_id.clone(),
                    format!("tool execution denied: {}", call.tool_name),
                ),
                ToolPermissionMode::Ask => {
                    let assistant_message_id = assistant_message_id.ok_or_else(|| {
                        AgentRuntimeError::Unsupported(
                            "subagent cannot await tool approval".to_string(),
                        )
                    })?;

                    self.append_tool_call_outputs(task_id, state, &mut outputs)
                        .await?;

                    let approval = self
                        .tool_permitter
                        .request(task_id, assistant_message_id, &call.call_id)
                        .await?;

                    self.task_repository.await_approval(task_id).await?;

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

                    return Ok(ToolRun::AwaitingApproval);
                }
                ToolPermissionMode::Allow => {
                    if call.tool_name == SubagentCall::TOOL_NAME {
                        match async {
                            let subagent_call =
                                SubagentCall::new(self.subagent_repository.list().await?);
                            let input = subagent_call
                                .parse(call.arguments.clone())
                                .map_err(AgentRuntimeError::Unsupported)?;

                            let mut results = Vec::new();

                            for (index, request) in input.tasks.into_iter().enumerate() {
                                let subagent = subagent_call
                                    .find(&request.profile)
                                    .cloned()
                                    .ok_or_else(|| {
                                        AgentRuntimeError::Unsupported(format!(
                                            "unsupported profile: {}",
                                            request.profile
                                        ))
                                    })?;

                                let profile_name = subagent.name.clone();

                                let mut instruction =
                                    self.instruction_service.build_agent_instruction();
                                instruction.push_str("\n\n# Child Agent Profile\n");
                                instruction.push_str(&subagent.instruction);
                                instruction.push_str(
                                    "\n\nComplete the delegated request and return a concise final result.",
                                );

                                let mut child_state = LoopState::Ephemeral {
                                    messages: vec![
                                        LlmMessage::system_text(instruction),
                                        LlmMessage::user_text(request.request),
                                    ],
                                };

                                let outcome = self
                                    .run_loop(
                                        task_id,
                                        Some(subagent),
                                        &mut child_state,
                                    )
                                    .await;

                                let (status, output, error) = match outcome {
                                    Ok(LoopOutcome::Completed(output)) => {
                                        (SubagentResultStatus::Completed, Some(output), None)
                                    }
                                    Ok(LoopOutcome::Stopped) => (
                                        SubagentResultStatus::Cancelled,
                                        None,
                                        Some("parent task cancelled".to_string()),
                                    ),
                                    Ok(LoopOutcome::AwaitingApproval) => (
                                        SubagentResultStatus::Failed,
                                        None,
                                        Some("subagent cannot await tool approval".to_string()),
                                    ),
                                    Err(err) => {
                                        (SubagentResultStatus::Failed, None, Some(err.to_string()))
                                    }
                                };

                                results.push(SubagentResult {
                                    index,
                                    profile: profile_name,
                                    status,
                                    output,
                                    error,
                                });
                            }

                            let output = SubagentCall::output(results);

                            Ok::<_, AgentRuntimeError>(ToolCallOutput::success(
                                call.call_id.clone(),
                                output,
                            ))
                        }
                        .await
                        {
                            Ok(output) => output,
                            Err(err) => {
                                ToolCallOutput::error(call.call_id.clone(), err.to_string())
                            }
                        }
                    } else {
                        match self.tool_executor.execute(call.clone()).await {
                            Ok(output) => output,
                            Err(err) => {
                                ToolCallOutput::error(call.call_id.clone(), err.to_string())
                            }
                        }
                    }
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

        self.append_tool_call_outputs(task_id, state, &mut outputs)
            .await?;
        Ok(ToolRun::Continue)
    }

    // make tool call output if pending approval exists
    async fn apply_approvals(&self, task_id: Uuid) -> Result<(), AgentRuntimeError> {
        let approvals = self.tool_permitter.ready(task_id).await?;

        for approval in approvals {
            self.apply_approval(approval).await?;
        }

        Ok(())
    }

    // approval status: approved/rejected -> (tool call) -> tool call output
    async fn apply_approval(&self, approval: ToolApproval) -> Result<Uuid, AgentRuntimeError> {
        if !approval.status.is_resolved() {
            return Err(AgentRuntimeError::ToolApprovalPending(approval.id));
        }

        let task = self
            .task_repository
            .find_by_id(approval.task_id)
            .await?
            .ok_or(AgentRuntimeError::TaskNotFound)?;

        if self.is_cancelled(task.id).await? {
            return Ok(task.id);
        }

        let message = self
            .message_repository
            .find_by_id(approval.message_id)
            .await?
            .ok_or(AgentRuntimeError::MessageNotFound(approval.message_id))?;

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
            "tool_call_finished",
            json!({
                "call_id": output.call_id,
                "output": output.output,
                "status": output.status.as_str(),
            }),
        )
        .await?;

        self.message_repository
            .save(task.id, Role::User, vec![output.into_message_content()])
            .await?;

        Ok(task.id)
    }

    pub async fn recover_approvals(&self) -> Result<u64, AgentRuntimeError> {
        let task_ids = self.tool_permitter.ready_tasks().await?;
        let count = task_ids.len() as u64;

        for task_id in task_ids {
            self.task_repository.resume_after_approval(task_id).await?;
        }

        Ok(count)
    }

    async fn complete(&self, task_id: Uuid, output: String) -> Result<(), AgentRuntimeError> {
        self.task_repository.complete(task_id).await?;
        self.emit(task_id, "task_completed", json!({ "output": output }))
            .await?;
        Ok(())
    }

    async fn fail(&self, task_id: Uuid, err: &AgentRuntimeError) -> Result<(), AgentRuntimeError> {
        let output = err.to_string();
        self.task_repository.fail(task_id, output.clone()).await?;
        self.emit(task_id, "task_failed", json!({ "error": output }))
            .await?;
        Ok(())
    }

    async fn tool_permission(
        &self,
        tool_name: &str,
        subagent: Option<&Subagent>,
    ) -> Result<ToolPermissionMode, AgentRuntimeError> {
        let (allowed_tools, allow_approval) = match subagent {
            Some(subagent) => (Some(subagent.allowed_tools.as_slice()), false),
            None => (None, true),
        };

        self.tool_permitter
            .mode(tool_name, allowed_tools, allow_approval)
            .await
            .map_err(Into::into)
    }

    async fn tool_specs(
        &self,
        subagent: Option<&Subagent>,
    ) -> Result<Vec<ToolSpec>, AgentRuntimeError> {
        let allowed_tools = subagent.map(|subagent| subagent.allowed_tools.as_slice());

        let extra_specs = if subagent.is_none() {
            let subagent_call = SubagentCall::new(self.subagent_repository.list().await?);
            subagent_call.tool_spec().into_iter().collect::<Vec<_>>()
        } else {
            Vec::new()
        };

        Ok(self.tool_executor.specs_for(allowed_tools, extra_specs))
    }

    async fn is_cancelled(&self, task_id: Uuid) -> Result<bool, AgentRuntimeError> {
        let task = self
            .task_repository
            .find_by_id(task_id)
            .await?
            .ok_or(AgentRuntimeError::TaskNotFound)?;

        Ok(task.status == TaskStatus::Cancelled)
    }
}

fn message_to_llm(message: Message) -> LlmMessage {
    LlmMessage::new(message.role, message.contents)
}
