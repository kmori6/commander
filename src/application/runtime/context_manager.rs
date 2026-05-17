use crate::application::error::agent_runtime_error::AgentRuntimeError;
use crate::domain::model::message::{Message, Role};
use crate::domain::model::task::Task;
use crate::domain::port::llm_provider::LlmMessage;
use crate::domain::repository::message_repository::MessageRepository;
use crate::domain::repository::task_repository::TaskRepository;
use crate::domain::service::instruction_service::InstructionService;

pub struct ContextManager<'a, T, M> {
    task_repository: &'a T,
    message_repository: &'a M,
    instruction_service: &'a InstructionService,
}

impl<'a, T, M> ContextManager<'a, T, M>
where
    T: TaskRepository,
    M: MessageRepository,
{
    pub fn new(
        task_repository: &'a T,
        message_repository: &'a M,
        instruction_service: &'a InstructionService,
    ) -> Self {
        Self {
            task_repository,
            message_repository,
            instruction_service,
        }
    }

    pub async fn build_for_task(
        &self,
        task: &Task,
        child_agent_instruction: Option<&str>,
    ) -> Result<Vec<LlmMessage>, AgentRuntimeError> {
        let mut messages = Vec::new();
        let mut instruction = self.instruction_service.build_agent_instruction();

        if let Some(child_agent_instruction) = child_agent_instruction {
            instruction.push_str("\n\n# Child Agent Profile\n");
            instruction.push_str(child_agent_instruction);
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

                append_task_messages(&mut messages, &session_task, task_messages);

                if is_current_task {
                    included_current_task = true;
                    break;
                }
            }

            if !included_current_task {
                let task_messages = self.message_repository.list_for_task(task.id).await?;
                append_task_messages(&mut messages, task, task_messages);
            }

            return Ok(messages);
        }

        let task_messages = self.message_repository.list_for_task(task.id).await?;
        append_task_messages(&mut messages, task, task_messages);

        Ok(messages)
    }
}

fn append_task_messages(messages: &mut Vec<LlmMessage>, task: &Task, task_messages: Vec<Message>) {
    let has_user_message = task_messages
        .iter()
        .any(|message| message.role == Role::User);

    if !has_user_message {
        messages.push(LlmMessage::user_text(task.request.clone()));
    }

    messages.extend(
        task_messages
            .into_iter()
            .map(|message| LlmMessage::new(message.role, message.contents)),
    );
}
