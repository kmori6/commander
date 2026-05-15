use crate::application::error::agent_runtime_error::AgentRuntimeError;
use crate::application::runtime::subagent;
use crate::application::runtime::subagent::{SubagentMode, SubagentProfile, Subagents};
use crate::domain::model::event::Event;
use crate::domain::model::message::{Message, MessageContent, Role};
use crate::domain::model::task::{Task, TaskSourceKind, TaskStatus};
use crate::domain::model::tool_call::{
    ToolApprovalStatus, ToolCall, ToolCallOutput, ToolPermissionMode, ToolSpec,
};
use crate::domain::port::llm_provider::{LlmMessage, LlmProvider, LlmRequest};
use crate::domain::repository::event_repository::EventRepository;
use crate::domain::repository::message_repository::MessageRepository;
use crate::domain::repository::task_repository::{CreateTask, TaskRepository};
use crate::domain::repository::token_usage_repository::{CreateTokenUsage, TokenUsageRepository};
use crate::domain::repository::tool_approval_repository::ToolApprovalRepository;
use crate::domain::repository::tool_permission_repository::ToolPermissionRepository;
use crate::domain::service::event_service::EventService;
use crate::domain::service::instruction_service::InstructionService;
use crate::domain::service::tool_executor::ToolExecutor;
use async_recursion::async_recursion;
use futures::future::join_all;
use serde_json::{Value, json};
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

const MAX_LLM_STEPS: usize = 30;

enum ToolCallRunOutcome {
    Continue,
    AwaitingApproval,
}

#[derive(Clone)]
struct RuntimeOptions {
    profile: Option<Arc<SubagentProfile>>,
    expose_subagent: bool,
}

impl RuntimeOptions {
    fn root() -> Self {
        Self {
            profile: None,
            expose_subagent: true,
        }
    }

    fn child(profile: Arc<SubagentProfile>) -> Self {
        Self {
            profile: Some(profile),
            expose_subagent: false,
        }
    }
}

pub struct AgentRuntime<L, T, M, E, U, P, A> {
    llm_provider: L,
    tool_executor: Arc<ToolExecutor>,
    task_repository: T,
    message_repository: M,
    event_repository: E,
    token_usage_repository: U,
    tool_permission_repository: P,
    tool_approval_repository: A,
    event_service: Arc<EventService>,
    instruction_service: Arc<InstructionService>,
    model: RwLock<String>,
}

impl<L, T, M, E, U, P, A> AgentRuntime<L, T, M, E, U, P, A>
where
    L: LlmProvider + 'static,
    T: TaskRepository + 'static,
    M: MessageRepository + 'static,
    E: EventRepository + 'static,
    U: TokenUsageRepository + 'static,
    P: ToolPermissionRepository + 'static,
    A: ToolApprovalRepository + 'static,
{
    pub fn new(
        llm_provider: L,
        tool_executor: Arc<ToolExecutor>,
        task_repository: T,
        message_repository: M,
        event_repository: E,
        token_usage_repository: U,
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
            event_repository,
            token_usage_repository,
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
        let event = self
            .event_repository
            .save(task_id, event_type, payload)
            .await?;

        self.event_service.publish(event.clone());

        Ok(event)
    }

    pub async fn run(
        self: Arc<Self>,
        task_id: Uuid,
        user_contents: Option<Vec<MessageContent>>,
    ) -> Result<(), AgentRuntimeError> {
        match self
            .clone()
            .execute(task_id, true, user_contents, RuntimeOptions::root())
            .await
        {
            Ok(()) => Ok(()),
            Err(err) => {
                if let Err(fail_err) = self.fail_task(task_id, &err).await {
                    log::warn!("failed to mark task {task_id} as failed: {fail_err}");
                }
                Err(err)
            }
        }
    }

    #[async_recursion]
    async fn execute(
        self: Arc<Self>,
        task_id: Uuid,
        emit_started: bool,
        user_contents: Option<Vec<MessageContent>>,
        options: RuntimeOptions,
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
                .build_llm_messages(&task, user_contents.as_ref(), &options)
                .await?;
            let model = self.model().await;

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
                    LlmRequest::new(model.clone(), messages).with_tools(self.tool_specs(&options)),
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

            let assistant_message = self
                .message_repository
                .save(task_id, Role::Assistant, response.message.contents.clone())
                .await?;

            self.token_usage_repository
                .save(CreateTokenUsage {
                    message_id: assistant_message.id,
                    model: model.clone(),
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
                .clone()
                .run_tool_calls(task_id, assistant_message.id, tool_calls, &options)
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

    pub async fn resume(self: Arc<Self>, approval_id: Uuid) -> Result<(), AgentRuntimeError> {
        let task_id = self.apply_approval(approval_id).await?;

        match self
            .clone()
            .execute(task_id, false, None, RuntimeOptions::root())
            .await
        {
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
            .find_by_id(message.task_id)
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
            .save(task.id, Role::User, vec![output.into_message_content()])
            .await?;

        Ok(task.id)
    }

    async fn build_llm_messages(
        &self,
        task: &Task,
        user_contents: Option<&Vec<MessageContent>>,
        options: &RuntimeOptions,
    ) -> Result<Vec<LlmMessage>, AgentRuntimeError> {
        let mut messages = Vec::new();

        let mut instruction = self.instruction_service.build_agent_instruction();

        if let Some(profile) = options.profile.as_ref() {
            instruction.push_str("\n\n# Child Agent Profile\n");
            instruction.push_str(&profile.instruction);
        }

        messages.push(LlmMessage::system_text(instruction));

        if let Some(session_id) = task.session_id {
            let session_tasks = self.task_repository.list_by_session_id(session_id).await?;
            let mut included_current_task = false;

            for session_task in session_tasks {
                let is_current_task = session_task.id == task.id;
                let task_messages = self
                    .message_repository
                    .list_for_task(session_task.id)
                    .await?;

                push_task_messages(
                    &mut messages,
                    &session_task,
                    is_current_task,
                    task_messages,
                    user_contents,
                );

                if is_current_task {
                    included_current_task = true;
                    break;
                }
            }

            if !included_current_task {
                let task_messages = self.message_repository.list_for_task(task.id).await?;

                push_task_messages(&mut messages, task, true, task_messages, user_contents);
            }

            return Ok(messages);
        }

        let task_messages = self.message_repository.list_for_task(task.id).await?;
        push_task_messages(&mut messages, task, true, task_messages, user_contents);

        Ok(messages)
    }

    async fn run_tool_calls(
        self: Arc<Self>,
        task_id: Uuid,
        assistant_message_id: Uuid,
        tool_calls: Vec<ToolCall>,
        options: &RuntimeOptions,
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

            // tool executor does not have subagent, so we need to resolve permission for subagent tool call here
            let mode = if is_subagent_runtime_tool(&call.tool_name) {
                if options.expose_subagent {
                    ToolPermissionMode::Allow
                } else {
                    ToolPermissionMode::Deny
                }
            } else {
                self.resolve_tool_permission(&call.tool_name, options)
                    .await?
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
                    if !outputs.is_empty() {
                        self.message_repository
                            .save(task_id, Role::User, outputs)
                            .await?;
                    }

                    let message_content_id = self
                        .message_repository
                        .find_tool_call_content_id(assistant_message_id, &call.call_id)
                        .await?
                        .ok_or_else(|| AgentRuntimeError::ToolCallNotFound(call.call_id.clone()))?;

                    let approval = self
                        .tool_approval_repository
                        .create_pending(task_id, message_content_id)
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
                ToolPermissionMode::Allow => {
                    if call.tool_name == subagent::SUBAGENT_TOOL_NAME {
                        match self
                            .clone()
                            .execute_subagent(task_id, call.arguments.clone())
                            .await
                        {
                            Ok(output) => ToolCallOutput::success(call.call_id.clone(), output),
                            Err(err) => {
                                ToolCallOutput::error(call.call_id.clone(), err.to_string())
                            }
                        }
                    } else if call.tool_name == subagent::SUBAGENT_STATUS_TOOL_NAME {
                        match self
                            .execute_subagent_status(task_id, call.arguments.clone())
                            .await
                        {
                            Ok(output) => ToolCallOutput::success(call.call_id.clone(), output),
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

        self.message_repository
            .save(task_id, Role::User, outputs)
            .await?;

        Ok(ToolCallRunOutcome::Continue)
    }

    async fn execute_subagent(
        self: Arc<Self>,
        parent_task_id: Uuid,
        arguments: Value,
    ) -> Result<Value, AgentRuntimeError> {
        let subagents = Subagents::load(self.instruction_service.workspace_root());
        let input = subagents
            .parse_input(arguments)
            .map_err(AgentRuntimeError::Unsupported)?;

        let mode = input.mode;
        let mut child_jobs = Vec::new();

        for (index, task_input) in input.tasks.into_iter().enumerate() {
            let profile = subagents
                .find(&task_input.profile)
                .cloned()
                .map(Arc::new)
                .ok_or_else(|| {
                    AgentRuntimeError::Unsupported(format!(
                        "unsupported profile: {}",
                        task_input.profile
                    ))
                })?;

            let child_task = self
                .task_repository
                .create(CreateTask {
                    request: task_input.request,
                    session_id: None,
                    source_kind: TaskSourceKind::Task,
                    source_message_id: None,
                    source_schedule_id: None,
                    parent_task_id: Some(parent_task_id),
                    scheduled_at: None,
                })
                .await?;

            child_jobs.push((index, child_task.id, profile));
        }

        if mode == SubagentMode::Spawn {
            let results = child_jobs
                .into_iter()
                .map(|(index, task_id, profile)| {
                    let runtime = self.clone();
                    let run_profile = profile.clone();

                    tokio::spawn(async move {
                        let result = runtime
                            .clone()
                            .execute(task_id, true, None, RuntimeOptions::child(run_profile))
                            .await;

                        if let Err(err) = result
                            && let Err(fail_err) = runtime.fail_task(task_id, &err).await
                        {
                            log::warn!(
                                "failed to mark spawned child task {task_id} as failed: {fail_err}"
                            );
                        }
                    });

                    subagent::SubagentTaskOutput {
                        index,
                        task_id: task_id.to_string(),
                        profile: profile.name.clone(),
                        status: subagent::SubagentTaskStatus::Spawned,
                        output: None,
                        error: None,
                    }
                })
                .collect();

            return serde_json::to_value(subagent::SubagentOutput { mode, results })
                .map_err(|err| AgentRuntimeError::Unsupported(err.to_string()));
        }

        let results = join_all(child_jobs.into_iter().map(|(index, task_id, profile)| {
            let runtime = self.clone();

            async move {
                let run_result = runtime
                    .clone()
                    .execute(task_id, true, None, RuntimeOptions::child(profile.clone()))
                    .await;

                if let Err(err) = run_result {
                    if let Err(fail_err) = runtime.fail_task(task_id, &err).await {
                        log::warn!("failed to mark child task {task_id} as failed: {fail_err}");
                    }

                    return subagent::SubagentTaskOutput {
                        index,
                        task_id: task_id.to_string(),
                        profile: profile.name.clone(),
                        status: subagent::SubagentTaskStatus::Failed,
                        output: None,
                        error: Some(err.to_string()),
                    };
                }

                match runtime.task_repository.find_by_id(task_id).await {
                    Ok(Some(task)) if task.status == TaskStatus::Completed => {
                        subagent::SubagentTaskOutput {
                            index,
                            task_id: task_id.to_string(),
                            profile: profile.name.clone(),
                            status: subagent::SubagentTaskStatus::Completed,
                            output: Some(task.output),
                            error: None,
                        }
                    }
                    Ok(Some(task)) if task.status == TaskStatus::Cancelled => {
                        subagent::SubagentTaskOutput {
                            index,
                            task_id: task_id.to_string(),
                            profile: profile.name.clone(),
                            status: subagent::SubagentTaskStatus::Cancelled,
                            output: None,
                            error: task.error,
                        }
                    }
                    Ok(Some(task)) => subagent::SubagentTaskOutput {
                        index,
                        task_id: task_id.to_string(),
                        profile: profile.name.clone(),
                        status: subagent::SubagentTaskStatus::Failed,
                        output: None,
                        error: task
                            .error
                            .or_else(|| Some("child task did not complete".to_string())),
                    },
                    Ok(None) => subagent::SubagentTaskOutput {
                        index,
                        task_id: task_id.to_string(),
                        profile: profile.name.clone(),
                        status: subagent::SubagentTaskStatus::Failed,
                        output: None,
                        error: Some("child task disappeared".to_string()),
                    },
                    Err(err) => subagent::SubagentTaskOutput {
                        index,
                        task_id: task_id.to_string(),
                        profile: profile.name.clone(),
                        status: subagent::SubagentTaskStatus::Failed,
                        output: None,
                        error: Some(err.to_string()),
                    },
                }
            }
        }))
        .await;

        serde_json::to_value(subagent::SubagentOutput { mode, results })
            .map_err(|err| AgentRuntimeError::Unsupported(err.to_string()))
    }

    async fn execute_subagent_status(
        &self,
        parent_task_id: Uuid,
        arguments: Value,
    ) -> Result<Value, AgentRuntimeError> {
        let input = subagent::parse_subagent_status_input(arguments)
            .map_err(AgentRuntimeError::Unsupported)?;

        let tasks = if let Some(task_id) = input.task_id {
            let task_id = Uuid::parse_str(&task_id).map_err(|err| {
                AgentRuntimeError::Unsupported(format!("invalid subagent task_id: {err}"))
            })?;

            let task = self
                .task_repository
                .find_by_id(task_id)
                .await?
                .ok_or(AgentRuntimeError::TaskNotFound)?;

            if task.parent_task_id != Some(parent_task_id) {
                return Err(AgentRuntimeError::Unsupported(format!(
                    "task is not a child of current task: {task_id}"
                )));
            }

            vec![subagent::SubagentStatusTaskOutput::from_task(&task)]
        } else {
            self.task_repository
                .list_by_parent_task_id(parent_task_id, None, input.limit)
                .await?
                .iter()
                .map(subagent::SubagentStatusTaskOutput::from_task)
                .collect()
        };

        serde_json::to_value(subagent::SubagentStatusOutput::new(tasks))
            .map_err(|err| AgentRuntimeError::Unsupported(err.to_string()))
    }

    async fn complete_task(&self, task_id: Uuid, output: String) -> Result<(), AgentRuntimeError> {
        self.task_repository.complete(task_id, output).await?;
        self.emit(task_id, "task_completed", json!({})).await?;
        Ok(())
    }

    async fn fail_task(
        &self,
        task_id: Uuid,
        err: &AgentRuntimeError,
    ) -> Result<(), AgentRuntimeError> {
        let output = err.to_string();

        self.task_repository.fail(task_id, output.clone()).await?;

        self.emit(task_id, "task_failed", json!({ "error": output }))
            .await?;

        Ok(())
    }

    async fn resolve_tool_permission(
        &self,
        tool_name: &str,
        options: &RuntimeOptions,
    ) -> Result<ToolPermissionMode, AgentRuntimeError> {
        // check for subagent allowed tools
        if let Some(profile) = options.profile.as_ref() {
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

    fn tool_specs(&self, options: &RuntimeOptions) -> Vec<ToolSpec> {
        let mut specs = self.tool_executor.specs();

        if let Some(profile) = options.profile.as_ref() {
            specs.retain(|spec| profile.allows_tool(&spec.name));
            return specs;
        }

        if options.expose_subagent {
            let subagents = Subagents::load(self.instruction_service.workspace_root());

            if let Some(spec) = subagents.tool_spec() {
                specs.push(spec);
            }

            specs.push(subagent::subagent_status_tool_spec());
        }

        specs
    }
}

fn is_subagent_runtime_tool(tool_name: &str) -> bool {
    tool_name == subagent::SUBAGENT_TOOL_NAME || tool_name == subagent::SUBAGENT_STATUS_TOOL_NAME
}

fn to_llm_message(message: Message) -> LlmMessage {
    LlmMessage::new(message.role, message.contents)
}

fn push_task_messages(
    messages: &mut Vec<LlmMessage>,
    task: &Task,
    is_current_task: bool,
    task_messages: Vec<Message>,
    user_contents: Option<&Vec<MessageContent>>,
) {
    let has_user_message = task_messages
        .iter()
        .any(|message| message.role == Role::User);

    if !has_user_message {
        push_task_request_message(messages, task, is_current_task, user_contents);
    }

    let mut replaced_user_message = false;

    for message in task_messages {
        if is_current_task
            && !replaced_user_message
            && message.role == Role::User
            && let Some(contents) = user_contents
        {
            messages.push(LlmMessage::new(Role::User, contents.clone()));
            replaced_user_message = true;
            continue;
        }

        messages.push(to_llm_message(message));
    }
}

fn push_task_request_message(
    messages: &mut Vec<LlmMessage>,
    task: &Task,
    is_current_task: bool,
    user_contents: Option<&Vec<MessageContent>>,
) {
    if is_current_task && let Some(contents) = user_contents {
        messages.push(LlmMessage::new(Role::User, contents.clone()));
        return;
    }

    messages.push(LlmMessage::user_text(task.request.clone()));
}
