use crate::application::error::agent_runtime_error::AgentRuntimeError;
use crate::application::runtime::subagent::{self, Profile, Registry};
use crate::domain::model::event::Event;
use crate::domain::model::message::{Message, MessageContent, Role};
use crate::domain::model::task::{Task, TaskStatus};
use crate::domain::model::tool_call::{
    ToolApproval, ToolApprovalStatus, ToolCall, ToolCallOutput, ToolPermissionMode, ToolSpec,
};
use crate::domain::port::llm_provider::{LlmMessage, LlmProvider, LlmRequest};
use crate::domain::repository::message_repository::MessageRepository;
use crate::domain::repository::task_repository::TaskRepository;
use crate::domain::repository::tool_approval_repository::ToolApprovalRepository;
use crate::domain::repository::tool_permission_repository::ToolPermissionRepository;
use crate::domain::service::compaction_service::{CompactionConfig, CompactionService};
use crate::domain::service::event_service::EventService;
use crate::domain::service::instruction_service::InstructionService;
use crate::domain::service::tool_executor::ToolExecutor;
use async_recursion::async_recursion;
use chrono::Utc;
use serde_json::{Value, json};
use std::sync::Arc;
use tokio::sync::RwLock;
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

#[derive(Clone)]
struct AgentScope {
    profile: Option<Profile>,
}

impl AgentScope {
    fn root() -> Self {
        Self { profile: None }
    }

    fn child(profile: Profile) -> Self {
        Self {
            profile: Some(profile),
        }
    }

    fn is_root(&self) -> bool {
        self.profile.is_none()
    }
}

pub struct AgentRuntime<L, T, M, P, A> {
    llm_provider: L,
    tool_executor: Arc<ToolExecutor>,
    task_repository: T,
    message_repository: M,
    tool_permission_repository: P,
    tool_approval_repository: A,
    event_service: Arc<EventService>,
    instruction_service: Arc<InstructionService>,
    model: RwLock<String>,
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
        task_repository: T,
        message_repository: M,
        event_service: Arc<EventService>,
        tool_permission_repository: P,
        tool_approval_repository: A,
        instruction_service: Arc<InstructionService>,
        model: String,
    ) -> Self {
        Self {
            llm_provider,
            tool_executor,
            task_repository,
            message_repository,
            event_service,
            tool_permission_repository,
            tool_approval_repository,
            instruction_service,
            model: RwLock::new(model),
        }
    }

    pub fn llm_provider(&self) -> &L {
        &self.llm_provider
    }

    pub async fn model(&self) -> String {
        self.model.read().await.clone()
    }

    pub async fn set_model(&self, model: impl Into<String>) {
        *self.model.write().await = model.into();
    }

    // agent runtime -> event service -> event handler -> sse event
    async fn emit(
        &self,
        task_id: Uuid,
        event_type: &str,
        payload: Value,
    ) -> Result<Event, AgentRuntimeError> {
        let event = Event {
            id: Uuid::new_v4(),
            task_id,
            event_type: event_type.to_string(),
            payload,
            created_at: Utc::now(),
        };

        self.event_service.publish(event.clone());

        Ok(event)
    }

    async fn build_llm_messages(
        &self,
        task: &Task,
        scope: &AgentScope,
        model: &str,
    ) -> Result<Vec<LlmMessage>, AgentRuntimeError> {
        let mut instruction = self.instruction_service.build_agent_instruction();

        if let Some(profile) = &scope.profile {
            instruction.push_str("\n\n# Child Agent Profile\n");
            instruction.push_str(&profile.instruction);
        }

        let mut messages = vec![LlmMessage::system_text(instruction)];
        let context_messages = self.context_messages(task).await?;
        messages.extend(self.compact_messages(model, context_messages).await?);

        Ok(messages)
    }

    async fn context_messages(&self, task: &Task) -> Result<Vec<Message>, AgentRuntimeError> {
        let mut messages = Vec::new();

        if let Some(session_id) = task.session_id {
            let mut included_current_task = false;

            for session_task in self.task_repository.list_by_session_id(session_id).await? {
                messages.extend(
                    self.message_repository
                        .list_for_task(session_task.id)
                        .await?,
                );

                if session_task.id == task.id {
                    included_current_task = true;
                    break;
                }
            }

            if !included_current_task {
                messages.extend(self.message_repository.list_for_task(task.id).await?);
            }

            return Ok(messages);
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
            .unwrap_or(256_000);

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

    async fn append_tool_outputs(
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

            self.emit(task_id, "task_started", json!({})).await?;

            if self.stop_cancelled(task_id).await? {
                return Ok(());
            }

            self.task_repository
                .update_status(task_id, TaskStatus::Running)
                .await?;

            self.apply_approvals(task_id).await?;

            let mut state = LoopState::Durable { task };

            match self
                .run_loop(task_id, AgentScope::root(), &mut state)
                .await?
            {
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
        scope: AgentScope,
        state: &mut LoopState,
    ) -> Result<LoopOutcome, AgentRuntimeError> {
        for step in 0..MAX_LLM_STEPS {
            if self.stop_cancelled(task_id).await? {
                return Ok(LoopOutcome::Stopped);
            }

            let model = self.model().await;

            let messages = match state {
                LoopState::Durable { task } => {
                    self.build_llm_messages(task, &scope, &model).await?
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
                    LlmRequest::new(model.clone(), messages).with_tools(self.tool_specs(&scope)),
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

            let tool_calls = response
                .message
                .contents
                .iter()
                .filter_map(ToolCall::from_message_content)
                .collect::<Vec<_>>();

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

            if tool_calls.is_empty() {
                if self.stop_cancelled(task_id).await? {
                    return Ok(LoopOutcome::Stopped);
                }

                return Ok(LoopOutcome::Completed(response.output_text("\n")));
            }

            match self
                .run_tools(task_id, assistant_message_id, tool_calls, &scope, state)
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
        scope: &AgentScope,
        state: &mut LoopState,
    ) -> Result<ToolRun, AgentRuntimeError> {
        let mut outputs = Vec::new();

        for call in tool_calls {
            if self.stop_cancelled(task_id).await? {
                self.append_tool_outputs(task_id, state, &mut outputs)
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

            // tool executor does not have subagent, so we need to resolve permission for subagent tool call here
            let mode = if is_runtime_tool(&call.tool_name) {
                if scope.is_root() {
                    ToolPermissionMode::Allow
                } else {
                    ToolPermissionMode::Deny
                }
            } else {
                self.tool_permission(&call.tool_name, scope).await?
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

                    self.append_tool_outputs(task_id, state, &mut outputs)
                        .await?;

                    let approval = self
                        .tool_approval_repository
                        .create_pending(task_id, assistant_message_id, &call.call_id)
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

                    return Ok(ToolRun::AwaitingApproval);
                }
                ToolPermissionMode::Allow => {
                    if call.tool_name == subagent::TOOL_NAME {
                        match async {
                            let registry =
                                Registry::load(self.instruction_service.workspace_root());
                            let input = registry
                                .parse_input(call.arguments.clone())
                                .map_err(AgentRuntimeError::Unsupported)?;

                            let mut results = Vec::new();

                            for (index, task_input) in input.tasks.into_iter().enumerate() {
                                let profile = registry
                                    .find(&task_input.profile)
                                    .cloned()
                                    .ok_or_else(|| {
                                        AgentRuntimeError::Unsupported(format!(
                                            "unsupported profile: {}",
                                            task_input.profile
                                        ))
                                    })?;

                                let profile_name = profile.name.clone();

                                let mut instruction =
                                    self.instruction_service.build_agent_instruction();
                                instruction.push_str("\n\n# Child Agent Profile\n");
                                instruction.push_str(&profile.instruction);
                                instruction.push_str(
                "\n\nComplete the delegated request and return a concise final result.",
            );

                                let mut child_state = LoopState::Ephemeral {
                                    messages: vec![
                                        LlmMessage::system_text(instruction),
                                        LlmMessage::user_text(task_input.request),
                                    ],
                                };

                                let outcome = self
                                    .run_loop(task_id, AgentScope::child(profile), &mut child_state)
                                    .await;

                                let (status, output, error) = match outcome {
                                    Ok(LoopOutcome::Completed(output)) => {
                                        (subagent::Status::Completed, Some(output), None)
                                    }
                                    Ok(LoopOutcome::Stopped) => (
                                        subagent::Status::Cancelled,
                                        None,
                                        Some("parent task cancelled".to_string()),
                                    ),
                                    Ok(LoopOutcome::AwaitingApproval) => (
                                        subagent::Status::Failed,
                                        None,
                                        Some("subagent cannot await tool approval".to_string()),
                                    ),
                                    Err(err) => {
                                        (subagent::Status::Failed, None, Some(err.to_string()))
                                    }
                                };

                                results.push(subagent::TaskOutput {
                                    index,
                                    profile: profile_name,
                                    status,
                                    output,
                                    error,
                                });
                            }

                            let output = serde_json::to_value(subagent::Output::new(results))
                                .map_err(|err| AgentRuntimeError::Unsupported(err.to_string()))?;

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

        self.append_tool_outputs(task_id, state, &mut outputs)
            .await?;
        Ok(ToolRun::Continue)
    }

    async fn apply_approvals(&self, task_id: Uuid) -> Result<(), AgentRuntimeError> {
        let approvals = self
            .tool_approval_repository
            .ready_for_task(task_id)
            .await?;

        for approval in approvals {
            self.apply_approval(approval).await?;
        }

        Ok(())
    }

    async fn apply_approval(&self, approval: ToolApproval) -> Result<Uuid, AgentRuntimeError> {
        if approval.status == ToolApprovalStatus::Pending {
            return Err(AgentRuntimeError::ToolApprovalPending(approval.id));
        }

        let message = self
            .message_repository
            .find_by_id(approval.message_id)
            .await?
            .ok_or(AgentRuntimeError::MessageNotFound(approval.message_id))?;

        let task = self
            .task_repository
            .find_by_id(approval.task_id)
            .await?
            .ok_or(AgentRuntimeError::TaskNotFound)?;

        if self.stop_cancelled(task.id).await? {
            return Ok(task.id);
        }

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
            .save(task.id, Role::User, vec![output.into_message_content()])
            .await?;

        Ok(task.id)
    }

    pub async fn recover_approvals(&self, limit: usize) -> Result<u64, AgentRuntimeError> {
        if limit == 0 {
            return Ok(0);
        }

        let mut total = 0;

        loop {
            let task_ids = self.tool_approval_repository.ready_task_ids(limit).await?;

            if task_ids.is_empty() {
                return Ok(total);
            }

            let count = task_ids.len();

            for task_id in task_ids {
                self.task_repository
                    .update_status(task_id, TaskStatus::Queued)
                    .await?;
                total += 1;
            }

            if count < limit {
                return Ok(total);
            }
        }
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
        scope: &AgentScope,
    ) -> Result<ToolPermissionMode, AgentRuntimeError> {
        // check for subagent allowed tools
        if let Some(profile) = scope.profile.as_ref() {
            if !profile.allows_tool(tool_name) {
                return Ok(ToolPermissionMode::Deny);
            }

            let mode = if let Some(permission) = self
                .tool_permission_repository
                .find_by_tool_name(tool_name)
                .await?
            {
                permission.mode
            } else {
                self.tool_executor
                    .default_permission(tool_name)
                    .unwrap_or(ToolPermissionMode::Deny)
            };

            return Ok(match mode {
                ToolPermissionMode::Allow => ToolPermissionMode::Allow,
                ToolPermissionMode::Ask | ToolPermissionMode::Deny => ToolPermissionMode::Deny,
            });
        }

        // check for root agent tools
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

    fn tool_specs(&self, scope: &AgentScope) -> Vec<ToolSpec> {
        let mut specs = self.tool_executor.specs();

        if let Some(profile) = scope.profile.as_ref() {
            specs.retain(|spec| profile.allows_tool(&spec.name));
            return specs;
        }

        let subagents = Registry::load(self.instruction_service.workspace_root());

        if let Some(spec) = subagents.tool_spec() {
            specs.push(spec);
        }

        specs
    }

    async fn stop_cancelled(&self, task_id: Uuid) -> Result<bool, AgentRuntimeError> {
        let task = self
            .task_repository
            .find_by_id(task_id)
            .await?
            .ok_or(AgentRuntimeError::TaskNotFound)?;

        Ok(task.status == TaskStatus::Cancelled)
    }
}

fn is_runtime_tool(tool_name: &str) -> bool {
    tool_name == subagent::TOOL_NAME
}

fn message_to_llm(message: Message) -> LlmMessage {
    LlmMessage::new(message.role, message.contents)
}
