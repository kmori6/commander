use crate::application::error::agent_runtime_error::AgentRuntimeError;
use crate::application::service::compaction_service::{CompactionConfig, CompactionService};
use crate::application::service::event_service::EventService;
use crate::application::service::instruction_service::InstructionService;
use crate::application::service::tool_service::ToolService;
use crate::domain::model::message::{Message, Role};
use crate::domain::model::task::{Task, TaskStatus};
use crate::domain::model::tool_call::{ToolCall, ToolCallOutput, ToolSpec};
use crate::domain::port::llm_provider::{LlmMessage, LlmProvider, LlmRequest, LlmResponse};
use crate::domain::repository::message_repository::MessageRepository;
use crate::domain::repository::task_repository::TaskRepository;
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

pub struct AgentRuntime<L, T, M> {
    llm_provider: L,
    tool_service: Arc<ToolService>,
    task_repository: T,
    message_repository: M,
    event_service: Arc<EventService>,
    instruction_service: Arc<InstructionService>,
}

impl<L, T, M> AgentRuntime<L, T, M>
where
    L: LlmProvider,
    T: TaskRepository,
    M: MessageRepository,
{
    pub fn new(
        llm_provider: L,
        tool_service: Arc<ToolService>,
        task_repository: T,
        message_repository: M,
        event_service: Arc<EventService>,
        instruction_service: Arc<InstructionService>,
    ) -> Self {
        Self {
            llm_provider,
            tool_service,
            task_repository,
            message_repository,
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

    // task -> instruction + compacted context -> llm messages
    async fn build_llm_messages(&self, task: &Task) -> Result<Vec<LlmMessage>, AgentRuntimeError> {
        let instruction = self.instruction_service.build_agent_instruction();

        let context_messages = if let Some(session_id) = task.session_id() {
            self.message_repository
                .list_for_session(session_id, Some(task.id))
                .await?
        } else {
            self.message_repository.list_for_task(task.id).await?
        };

        let mut messages = vec![LlmMessage::system_text(instruction)];
        messages.extend(self.compact_messages(context_messages).await?);

        Ok(messages)
    }

    async fn compact_messages(
        &self,
        messages: Vec<Message>,
    ) -> Result<Vec<LlmMessage>, AgentRuntimeError> {
        let context_window = self.llm_provider.context_window().try_into().map_err(|_| {
            AgentRuntimeError::Unsupported("unsupported context window".to_string())
        })?;

        let service = CompactionService::new(CompactionConfig::for_window(context_window));

        let Some(result) = service
            .compact(&self.llm_provider, messages.clone())
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
    // run -> run_loop -> run_tools
    pub async fn run(&self, task_id: Uuid) -> Result<(), AgentRuntimeError> {
        let task = self
            .task_repository
            .find_by_id(task_id)
            .await?
            .ok_or(AgentRuntimeError::TaskNotFound)?;

        if task.status != TaskStatus::Running {
            return Err(AgentRuntimeError::Unsupported(format!(
                "agent runtime requires running task, got {}",
                task.status.as_str()
            )));
        }

        let result = async {
            self.emit(task_id, "task_started", json!({})).await?;

            if self.is_cancelled(task_id).await? {
                return Ok(());
            }

            match self.run_loop(task_id, &task).await? {
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
    async fn run_loop(&self, task_id: Uuid, task: &Task) -> Result<LoopOutcome, AgentRuntimeError> {
        for step in 0..MAX_LLM_STEPS {
            if self.is_cancelled(task_id).await? {
                return Ok(LoopOutcome::Stopped);
            }

            let model = self.llm_provider.model().to_string();
            let messages = self.build_llm_messages(task).await?;

            let response = self
                .call_llm(
                    task_id,
                    &model,
                    step + 1,
                    messages,
                    self.tool_service.list_tools(),
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

            match self.run_tools(task_id, tool_calls).await? {
                ToolRun::Continue => {}
                ToolRun::Stopped => return Ok(LoopOutcome::Stopped),
            }
        }

        Err(AgentRuntimeError::Unsupported(format!(
            "maximum LLM steps exceeded: {MAX_LLM_STEPS}"
        )))
    }

    // tool calls (durable with database, for root agent)
    async fn run_tools(
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

            let output = match self.tool_service.execute(call.clone()).await {
                Ok(output) => output,
                Err(err) => ToolCallOutput::error(call.call_id.clone(), err.to_string()),
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
            .response(LlmRequest::new(messages).with_tools(tools))
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
}
