use crate::application::error::agent_runtime_error::AgentRuntimeError;
use crate::application::runtime::subagent_tool::SubagentTool;
use crate::application::service::compaction_service::{CompactionConfig, CompactionService};
use crate::application::service::event_service::EventService;
use crate::application::service::instruction_service::InstructionService;
use crate::application::service::tool_service::ToolService;
use crate::domain::model::message::{Message, Role};
use crate::domain::model::subagent::Subagent;
use crate::domain::model::task::{Task, TaskStatus};
use crate::domain::model::tool_call::{ToolCall, ToolCallOutput, ToolPermissionMode, ToolSpec};
use crate::domain::port::llm_provider::{LlmMessage, LlmProvider, LlmRequest, LlmResponse};
use crate::domain::repository::message_repository::MessageRepository;
use crate::domain::repository::subagent_repository::SubagentRepository;
use crate::domain::repository::task_repository::TaskRepository;
use crate::domain::repository::tool_permission_repository::ToolPermissionRepository;
use serde_json::{Value, json};
use std::sync::Arc;
use uuid::Uuid;

const MAX_LLM_STEPS: usize = 30;

enum LoopOutcome {
    Completed(String),
    Stopped,
}

enum ToolRun {
    Continue,
    Stopped,
}

pub struct AgentRuntime<L, T, M, S, P> {
    llm_provider: L,
    tool_service: Arc<ToolService<P>>,
    task_repository: T,
    message_repository: M,
    subagent_repository: S,
    event_service: Arc<EventService>,
    instruction_service: Arc<InstructionService>,
}

impl<L, T, M, S, P> AgentRuntime<L, T, M, S, P>
where
    L: LlmProvider,
    T: TaskRepository,
    M: MessageRepository,
    S: SubagentRepository,
    P: ToolPermissionRepository,
{
    pub fn new(
        llm_provider: L,
        tool_service: Arc<ToolService<P>>,
        task_repository: T,
        message_repository: M,
        subagent_repository: S,
        event_service: Arc<EventService>,
        instruction_service: Arc<InstructionService>,
    ) -> Self {
        Self {
            llm_provider,
            tool_service,
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

    // task + model -> instruction + compacted context -> llm messages
    async fn build_llm_messages(
        &self,
        task: &Task,
        model: &str,
    ) -> Result<Vec<LlmMessage>, AgentRuntimeError> {
        let instruction = self.instruction_service.build_agent_instruction();

        let context_messages = if let Some(session_id) = task.session_id() {
            self.message_repository
                .list_for_session(session_id, Some(task.id))
                .await?
        } else {
            self.message_repository.list_for_task(task.id).await?
        };

        let mut messages = vec![LlmMessage::system_text(instruction)];
        messages.extend(self.compact_messages(model, context_messages).await?);

        Ok(messages)
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
            return Ok(messages
                .into_iter()
                .map(|message| LlmMessage::new(message.role, message.contents))
                .collect());
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

        compacted.extend(
            retained
                .into_iter()
                .map(|message| LlmMessage::new(message.role, message.contents)),
        );
        Ok(compacted)
    }

    // root agent entry
    // run
    //   -> run_durable_loop
    //        -> run_durable_tools
    //             -> run_subagent_call
    //                  -> run_ephemeral_loop
    //                       -> run_ephemeral_tools
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

            match self.run_durable_loop(task_id, &task).await? {
                LoopOutcome::Completed(output) => self.complete(task_id, output).await?,
                LoopOutcome::Stopped => {}
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

    async fn is_cancelled(&self, task_id: Uuid) -> Result<bool, AgentRuntimeError> {
        let task = self
            .task_repository
            .find_by_id(task_id)
            .await?
            .ok_or(AgentRuntimeError::TaskNotFound)?;

        Ok(task.status == TaskStatus::Cancelled)
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

    // agent loop (durable with database, for root agent)
    async fn run_durable_loop(
        &self,
        task_id: Uuid,
        task: &Task,
    ) -> Result<LoopOutcome, AgentRuntimeError> {
        for step in 0..MAX_LLM_STEPS {
            if self.is_cancelled(task_id).await? {
                return Ok(LoopOutcome::Stopped);
            }

            let model = self.llm_provider.current_model_id().await?;
            let messages = self.build_llm_messages(task, &model).await?;

            let response = self
                .call_llm(
                    task_id,
                    &model,
                    step + 1,
                    messages,
                    self.llm_tool_specs(None).await?,
                )
                .await?;

            self.message_repository
                .save_response(
                    task_id,
                    response.message.contents.clone(),
                    &model,
                    response.usage,
                )
                .await?;

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

            match self.run_durable_tools(task_id, tool_calls).await? {
                ToolRun::Continue => {}
                ToolRun::Stopped => return Ok(LoopOutcome::Stopped),
            }
        }

        Err(AgentRuntimeError::Unsupported(format!(
            "maximum LLM steps exceeded: {MAX_LLM_STEPS}"
        )))
    }

    // tool calls (durable with database, for root agent)
    async fn run_durable_tools(
        &self,
        task_id: Uuid,
        tool_calls: Vec<ToolCall>,
    ) -> Result<ToolRun, AgentRuntimeError> {
        let mut outputs = Vec::new();

        for call in tool_calls {
            if self.is_cancelled(task_id).await? {
                if !outputs.is_empty() {
                    self.message_repository
                        .save(task_id, Role::User, std::mem::take(&mut outputs))
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

            // Root agent runs tools directly; approvals and root permissions are being phased out.
            let mode = ToolPermissionMode::Allow;

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

            let output = if call.tool_name == SubagentTool::name() {
                match self.run_subagent_call(task_id, &call).await {
                    Ok(output) => output,
                    Err(err) => ToolCallOutput::error(call.call_id.clone(), err.to_string()),
                }
            } else {
                match self.tool_service.execute(call.clone()).await {
                    Ok(output) => output,
                    Err(err) => ToolCallOutput::error(call.call_id.clone(), err.to_string()),
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

        if !outputs.is_empty() {
            self.message_repository
                .save(task_id, Role::User, outputs)
                .await?;
        }
        Ok(ToolRun::Continue)
    }

    // run subagent and return output as `tool call output`
    async fn run_subagent_call(
        &self,
        task_id: Uuid,
        call: &ToolCall,
    ) -> Result<ToolCallOutput, AgentRuntimeError> {
        let profiles = self.subagent_repository.list().await?;
        let tasks = SubagentTool::parse_tasks(&profiles, call.arguments.clone())
            .map_err(AgentRuntimeError::Unsupported)?;

        let mut results = Vec::new();

        for (index, subagent, request) in tasks {
            let profile_name = subagent.name.clone();

            let mut instruction = self.instruction_service.build_agent_instruction();
            instruction.push_str("\n\n# Child Agent Profile\n");
            instruction.push_str(&subagent.instruction);
            instruction
                .push_str("\n\nComplete the delegated request and return a concise final result.");

            let initial_messages = vec![
                LlmMessage::system_text(instruction),
                LlmMessage::user_text(request),
            ];

            let outcome = self
                .run_ephemeral_loop(task_id, &subagent, initial_messages)
                .await;

            let (status, output, error) = match outcome {
                Ok(LoopOutcome::Completed(output)) => ("completed", Some(output), None),
                Ok(LoopOutcome::Stopped) => {
                    ("cancelled", None, Some("parent task cancelled".to_string()))
                }
                Err(err) => ("failed", None, Some(err.to_string())),
            };

            let mut result = json!({
                "index": index,
                "profile": profile_name,
                "status": status,
            });

            if let Some(output) = output {
                result["output"] = json!(output);
            }

            if let Some(error) = error {
                result["error"] = json!(error);
            }

            results.push(result);
        }

        Ok(ToolCallOutput::success(
            call.call_id.clone(),
            json!({ "results": results }),
        ))
    }

    // agent loop (in-memory, for subagent)
    async fn run_ephemeral_loop(
        &self,
        task_id: Uuid,
        subagent: &Subagent,
        mut messages: Vec<LlmMessage>,
    ) -> Result<LoopOutcome, AgentRuntimeError> {
        for step in 0..MAX_LLM_STEPS {
            if self.is_cancelled(task_id).await? {
                return Ok(LoopOutcome::Stopped);
            }

            let model = self.llm_provider.current_model_id().await?;

            let response = self
                .call_llm(
                    task_id,
                    &model,
                    step + 1,
                    messages.clone(),
                    self.llm_tool_specs(Some(subagent)).await?,
                )
                .await?;

            messages.push(response.message.clone());

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
                .run_ephemeral_tools(task_id, subagent, tool_calls, &mut messages)
                .await?
            {
                ToolRun::Continue => {}
                ToolRun::Stopped => return Ok(LoopOutcome::Stopped),
            }
        }

        Err(AgentRuntimeError::Unsupported(format!(
            "maximum LLM steps exceeded: {MAX_LLM_STEPS}"
        )))
    }

    // tool calls (in-memory, for subagent)
    async fn run_ephemeral_tools(
        &self,
        task_id: Uuid,
        subagent: &Subagent,
        tool_calls: Vec<ToolCall>,
        messages: &mut Vec<LlmMessage>,
    ) -> Result<ToolRun, AgentRuntimeError> {
        let mut outputs = Vec::new();

        for call in tool_calls {
            if self.is_cancelled(task_id).await? {
                if !outputs.is_empty() {
                    messages.push(LlmMessage::new(Role::User, std::mem::take(&mut outputs)));
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

            let mode = if call.tool_name == SubagentTool::name() {
                ToolPermissionMode::Deny
            } else {
                self.tool_service
                    .permission_mode(&call.tool_name, Some(subagent.allowed_tools.as_slice()))
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
                ToolPermissionMode::Allow => match self.tool_service.execute(call.clone()).await {
                    Ok(output) => output,
                    Err(err) => ToolCallOutput::error(call.call_id.clone(), err.to_string()),
                },
                _ => ToolCallOutput::error(
                    call.call_id.clone(),
                    format!("tool execution denied: {}", call.tool_name),
                ),
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

        if !outputs.is_empty() {
            messages.push(LlmMessage::new(Role::User, outputs));
        }

        Ok(ToolRun::Continue)
    }

    // emit event (llm_started) -> call llm provider -> emit event (llm_finished)
    async fn call_llm(
        &self,
        task_id: Uuid,
        model: &str,
        step: usize,
        messages: Vec<LlmMessage>,
        tools: Vec<ToolSpec>,
    ) -> Result<LlmResponse, AgentRuntimeError> {
        self.emit(
            task_id,
            "llm_started",
            json!({
                "model": model,
                "step": step,
            }),
        )
        .await?;

        let response = match self
            .llm_provider
            .respond(LlmRequest::new(model.to_string(), messages).with_tools(tools))
            .await
        {
            Ok(response) => response,
            Err(err) => {
                self.emit(
                    task_id,
                    "llm_failed",
                    json!({
                        "model": model,
                        "step": step,
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
                "model": model,
                "step": step,
                "input_tokens": response.usage.input_tokens,
                "output_tokens": response.usage.output_tokens,
                "cache_read_tokens": response.usage.cache_read_tokens,
                "cache_write_tokens": response.usage.cache_write_tokens,
            }),
        )
        .await?;

        Ok(response)
    }

    // root: tools + subagent_call
    // subagent: allowed_tools
    async fn llm_tool_specs(
        &self,
        subagent: Option<&Subagent>,
    ) -> Result<Vec<ToolSpec>, AgentRuntimeError> {
        match subagent {
            None => {
                let profiles = self.subagent_repository.list().await?;
                Ok(self
                    .tool_service
                    .specs_for(None, SubagentTool::spec(&profiles)))
            }
            Some(subagent) => Ok(self.tool_service.specs_for(
                Some(subagent.allowed_tools.as_slice()),
                std::iter::empty::<ToolSpec>(),
            )),
        }
    }
}
