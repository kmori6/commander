use crate::application::error::agent_runtime_error::AgentRuntimeError;
use crate::application::runtime::context_manager::ContextManager;
use crate::application::runtime::subagent_tool::{self, SubagentProfile, Subagents};
use crate::application::runtime::task_status_tool;
use crate::domain::model::event::Event;
use crate::domain::model::message::Role;
use crate::domain::model::task::{Task, TaskSourceKind, TaskStatus};
use crate::domain::model::tool_call::{
    ToolApprovalStatus, ToolCall, ToolCallOutput, ToolPermissionMode, ToolSpec,
};
use crate::domain::port::llm_provider::{LlmProvider, LlmRequest};
use crate::domain::repository::event_repository::EventRepository;
use crate::domain::repository::message_repository::MessageRepository;
use crate::domain::repository::task_repository::{CreateTask, TaskRepository};
use crate::domain::repository::token_usage_repository::{CreateTokenUsage, TokenUsageRepository};
use crate::domain::repository::tool_approval_repository::ToolApprovalRepository;
use crate::domain::repository::tool_permission_repository::ToolPermissionRepository;
use crate::domain::service::event_service::EventService;
use crate::domain::service::instruction_service::InstructionService;
use crate::domain::service::tool_executor::ToolExecutor;
use serde_json::{Value, json};
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

const MAX_LLM_STEPS: usize = 30;

enum ToolRun {
    Continue,
    AwaitingApproval,
    AwaitingChild,
    Stopped,
}

#[derive(Clone)]
struct AgentScope {
    profile: Option<SubagentProfile>,
}

impl AgentScope {
    fn root() -> Self {
        Self { profile: None }
    }

    fn child(profile: SubagentProfile) -> Self {
        Self {
            profile: Some(profile),
        }
    }

    fn is_root(&self) -> bool {
        self.profile.is_none()
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

    // root agent or subscribed subagent
    fn task_scope(&self, task: &Task) -> Result<AgentScope, AgentRuntimeError> {
        let Some(profile_name) = task.subagent_profile.as_deref() else {
            return Ok(AgentScope::root());
        };

        let subagents = Subagents::load(self.instruction_service.workspace_root());
        let profile = subagents.find(profile_name).cloned().ok_or_else(|| {
            AgentRuntimeError::Unsupported(format!("unsupported subagent profile: {profile_name}"))
        })?;

        Ok(AgentScope::child(profile))
    }

    pub async fn run(self: Arc<Self>, task_id: Uuid) -> Result<(), AgentRuntimeError> {
        match self.run_loop(task_id, true).await {
            Ok(()) => Ok(()),
            Err(err) => {
                if let Err(fail_err) = self.fail(task_id, &err).await {
                    log::warn!("failed to mark task {task_id} as failed: {fail_err}");
                }
                Err(err)
            }
        }
    }

    async fn run_loop(&self, task_id: Uuid, emit_started: bool) -> Result<(), AgentRuntimeError> {
        let task = self
            .task_repository
            .find_by_id(task_id)
            .await?
            .ok_or(AgentRuntimeError::TaskNotFound)?;
        let scope = self.task_scope(&task)?;

        if emit_started {
            self.emit(task_id, "task_started", json!({})).await?;
        }

        if self.stop_cancelled(task_id, "before_start").await? {
            return Ok(());
        }

        self.task_repository
            .update_status(task_id, TaskStatus::Running)
            .await?;

        for step in 0..MAX_LLM_STEPS {
            if self.stop_cancelled(task_id, "before_llm").await? {
                return Ok(());
            }

            // LLM call
            let child_agent_instruction = scope
                .profile
                .as_ref()
                .map(|profile| profile.instruction.as_str());
            let messages = ContextManager::new(
                &self.task_repository,
                &self.message_repository,
                self.instruction_service.as_ref(),
            )
            .build_for_task(&task, child_agent_instruction)
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
                if self.stop_cancelled(task_id, "before_complete").await? {
                    return Ok(());
                }

                self.complete(task_id, response.output_text("\n")).await?;
                return Ok(());
            }

            match self
                .run_tools(task_id, assistant_message.id, tool_calls, &scope)
                .await?
            {
                ToolRun::Continue => {}
                ToolRun::AwaitingApproval => return Ok(()),
                ToolRun::AwaitingChild => return Ok(()),
                ToolRun::Stopped => return Ok(()),
            }
        }

        Err(AgentRuntimeError::Unsupported(format!(
            "maximum LLM steps exceeded: {MAX_LLM_STEPS}"
        )))
    }

    pub async fn resume(self: Arc<Self>, approval_id: Uuid) -> Result<(), AgentRuntimeError> {
        let task_id = self.apply_approval(approval_id).await?;

        match self.run_loop(task_id, false).await {
            Ok(()) => Ok(()),
            Err(err) => {
                if let Err(fail_err) = self.fail(task_id, &err).await {
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

        if self
            .stop_cancelled(task.id, "before_approval_resume")
            .await?
        {
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

    async fn run_tools(
        &self,
        task_id: Uuid,
        assistant_message_id: Uuid,
        tool_calls: Vec<ToolCall>,
        scope: &AgentScope,
    ) -> Result<ToolRun, AgentRuntimeError> {
        let mut outputs = Vec::new();

        for call in tool_calls {
            if self.stop_cancelled(task_id, "before_tool_call").await? {
                if !outputs.is_empty() {
                    self.message_repository
                        .save(task_id, Role::User, outputs)
                        .await?;
                }

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

                    return Ok(ToolRun::AwaitingApproval);
                }
                ToolPermissionMode::Allow => {
                    if call.tool_name == subagent_tool::SUBAGENT_TOOL_NAME {
                        if !outputs.is_empty() {
                            self.message_repository
                                .save(task_id, Role::User, outputs)
                                .await?;
                        }

                        self.start_children(
                            task_id,
                            assistant_message_id,
                            &call.call_id,
                            call.arguments.clone(),
                        )
                        .await?;

                        return Ok(ToolRun::AwaitingChild);
                    } else if call.tool_name == task_status_tool::TASK_STATUS_TOOL_NAME {
                        match self.task_status(call.arguments.clone()).await {
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

        Ok(ToolRun::Continue)
    }

    async fn start_children(
        &self,
        parent_task_id: Uuid,
        assistant_message_id: Uuid,
        source_tool_call_id: &str,
        arguments: Value,
    ) -> Result<(), AgentRuntimeError> {
        let subagents = Subagents::load(self.instruction_service.workspace_root());
        let input = subagents
            .parse_input(arguments)
            .map_err(AgentRuntimeError::Unsupported)?;

        for task_input in input.tasks {
            let profile = subagents.find(&task_input.profile).ok_or_else(|| {
                AgentRuntimeError::Unsupported(format!(
                    "unsupported profile: {}",
                    task_input.profile
                ))
            })?;

            self.task_repository
                .create(CreateTask {
                    request: task_input.request,
                    session_id: None,
                    source_kind: TaskSourceKind::Task,
                    source_message_id: Some(assistant_message_id),
                    source_schedule_id: None,
                    source_tool_call_id: Some(source_tool_call_id.to_string()),
                    subagent_profile: Some(profile.name.clone()),
                    parent_task_id: Some(parent_task_id),
                    scheduled_at: None,
                })
                .await?;
        }

        self.task_repository
            .update_status(parent_task_id, TaskStatus::AwaitingChild)
            .await?;

        self.emit(
            parent_task_id,
            "task_awaiting_child",
            json!({
                "source_tool_call_id": source_tool_call_id,
            }),
        )
        .await?;

        Ok(())
    }

    pub async fn recover_children(&self, limit: usize) -> Result<(), AgentRuntimeError> {
        loop {
            let children = self.task_repository.list_joinable_children(limit).await?;

            if children.is_empty() {
                return Ok(());
            }

            let count = children.len();

            for child in children {
                self.join_parent(&child).await?;
            }

            if count < limit {
                return Ok(());
            }
        }
    }

    async fn task_status(&self, arguments: Value) -> Result<Value, AgentRuntimeError> {
        let input = task_status_tool::parse_task_status_input(arguments)
            .map_err(AgentRuntimeError::Unsupported)?;

        let tasks = if let Some(task_id) = input.task_id {
            let task_id = Uuid::parse_str(&task_id)
                .map_err(|err| AgentRuntimeError::Unsupported(format!("invalid task_id: {err}")))?;

            let task = self
                .task_repository
                .find_by_id(task_id)
                .await?
                .ok_or(AgentRuntimeError::TaskNotFound)?;

            vec![task_status_tool::TaskStatusTaskOutput::from_task(
                &task, true,
            )]
        } else if let Some(parent_task_id) = input.parent_task_id {
            let parent_task_id = Uuid::parse_str(&parent_task_id).map_err(|err| {
                AgentRuntimeError::Unsupported(format!("invalid parent_task_id: {err}"))
            })?;

            self.task_repository
                .list_children(parent_task_id, input.status, input.limit)
                .await?
                .iter()
                .map(|task| task_status_tool::TaskStatusTaskOutput::from_task(task, false))
                .collect()
        } else {
            self.task_repository
                .list_recent(input.status, input.limit)
                .await?
                .iter()
                .map(|task| task_status_tool::TaskStatusTaskOutput::from_task(task, false))
                .collect()
        };

        serde_json::to_value(task_status_tool::TaskStatusOutput::new(tasks))
            .map_err(|err| AgentRuntimeError::Unsupported(err.to_string()))
    }

    async fn complete(&self, task_id: Uuid, output: String) -> Result<(), AgentRuntimeError> {
        let task = self.task_repository.complete(task_id, output).await?;
        self.emit(task_id, "task_completed", json!({})).await?;
        self.try_join(&task).await;
        Ok(())
    }

    async fn fail(&self, task_id: Uuid, err: &AgentRuntimeError) -> Result<(), AgentRuntimeError> {
        let output = err.to_string();

        let task = self.task_repository.fail(task_id, output.clone()).await?;

        self.emit(task_id, "task_failed", json!({ "error": output }))
            .await?;

        self.try_join(&task).await;

        Ok(())
    }

    async fn join_parent(&self, child: &Task) -> Result<(), AgentRuntimeError> {
        let Some(parent_task_id) = child.parent_task_id else {
            return Ok(());
        };

        let Some(source_tool_call_id) = child.source_tool_call_id.as_deref() else {
            return Ok(());
        };

        let parent = self
            .task_repository
            .find_by_id(parent_task_id)
            .await?
            .ok_or(AgentRuntimeError::TaskNotFound)?;

        if !matches!(
            parent.status,
            TaskStatus::AwaitingChild | TaskStatus::CancelRequested
        ) {
            return Ok(());
        }

        let children = self
            .task_repository
            .list_child_group(parent_task_id, source_tool_call_id)
            .await?;

        if children.is_empty() || children.iter().any(|task| !task.status.is_terminal()) {
            return Ok(());
        }

        if parent.status == TaskStatus::CancelRequested {
            self.task_repository.cancel(parent_task_id).await?;
            self.emit(
                parent_task_id,
                "task_cancelled",
                json!({ "checkpoint": "after_child_tasks" }),
            )
            .await?;
            return Ok(());
        }

        if self
            .message_repository
            .has_tool_output(parent_task_id, source_tool_call_id)
            .await?
        {
            self.task_repository
                .update_status(parent_task_id, TaskStatus::Queued)
                .await?;
            return Ok(());
        }

        let output = serde_json::to_value(subagent_tool::SubagentOutput::from_tasks(&children))
            .map_err(|err| AgentRuntimeError::Unsupported(err.to_string()))?;
        let tool_output = ToolCallOutput::success(source_tool_call_id.to_string(), output);

        self.emit(
            parent_task_id,
            "tool_call_finished",
            json!({
                "call_id": &tool_output.call_id,
                "output": &tool_output.output,
                "status": tool_output.status.as_str(),
            }),
        )
        .await?;

        self.message_repository
            .save(
                parent_task_id,
                Role::User,
                vec![tool_output.into_message_content()],
            )
            .await?;

        self.emit(
            parent_task_id,
            "child_tasks_joined",
            json!({
                "source_tool_call_id": source_tool_call_id,
                "child_count": children.len(),
            }),
        )
        .await?;

        self.task_repository
            .update_status(parent_task_id, TaskStatus::Queued)
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

        let subagents = Subagents::load(self.instruction_service.workspace_root());

        if let Some(spec) = subagents.tool_spec() {
            specs.push(spec);
        }

        specs.push(task_status_tool::task_status_tool_spec());

        specs
    }

    async fn stop_cancelled(
        &self,
        task_id: Uuid,
        checkpoint: &str,
    ) -> Result<bool, AgentRuntimeError> {
        let task = self
            .task_repository
            .find_by_id(task_id)
            .await?
            .ok_or(AgentRuntimeError::TaskNotFound)?;

        if task.status == TaskStatus::Cancelled {
            return Ok(true);
        }

        if task.status == TaskStatus::CancelRequested {
            self.cancel(task_id, checkpoint).await?;
            return Ok(true);
        }

        Ok(false)
    }

    async fn cancel(&self, task_id: Uuid, checkpoint: &str) -> Result<(), AgentRuntimeError> {
        let task = self.task_repository.cancel(task_id).await?;
        self.emit(
            task_id,
            "task_cancelled",
            json!({ "checkpoint": checkpoint }),
        )
        .await?;
        self.try_join(&task).await;
        Ok(())
    }

    async fn try_join(&self, task: &Task) {
        if let Err(err) = self.join_parent(task).await {
            log::warn!("failed to join parent for task {}: {err}", task.id);
        }
    }
}

fn is_runtime_tool(tool_name: &str) -> bool {
    tool_name == subagent_tool::SUBAGENT_TOOL_NAME
        || tool_name == task_status_tool::TASK_STATUS_TOOL_NAME
}
