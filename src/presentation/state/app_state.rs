use std::sync::Arc;

use crate::application::runtime::agent_runtime::AgentRuntime;
use crate::application::service::event_service::EventService;
use crate::application::service::instruction_service::InstructionService;
use crate::application::service::tool_service::ToolService;
use crate::application::usecase::message_usecase::MessageUsecase;
use crate::application::usecase::schedule_usecase::ScheduleUsecase;
use crate::application::usecase::session_usecase::SessionUsecase;
use crate::application::usecase::task_usecase::TaskUsecase;
use crate::infrastructure::llm::openai_llm_provider::OpenaiLlmProvider;
use crate::infrastructure::persistence::file_schedule_repository::FileScheduleRepository;
use crate::infrastructure::persistence::postgres_message_repository::PostgresMessageRepository;
use crate::infrastructure::persistence::postgres_session_repository::PostgresSessionRepository;
use crate::infrastructure::persistence::postgres_task_repository::PostgresTaskRepository;

#[derive(Clone)]
pub struct AppState {
    // services
    pub event_service: Arc<EventService>,
    pub instruction_service: Arc<InstructionService>,
    pub tool_service: Arc<ToolService>,
    // usecases
    pub session_usecase: Arc<SessionUsecase<PostgresSessionRepository>>,
    pub message_usecase: Arc<
        MessageUsecase<
            PostgresMessageRepository,
            PostgresSessionRepository,
            PostgresTaskRepository,
        >,
    >,
    pub task_usecase: Arc<TaskUsecase<PostgresTaskRepository, PostgresMessageRepository>>,
    pub schedule_usecase: Arc<
        ScheduleUsecase<FileScheduleRepository, PostgresTaskRepository, PostgresMessageRepository>,
    >,
    // runtimes
    pub agent_runtime:
        Arc<AgentRuntime<OpenaiLlmProvider, PostgresTaskRepository, PostgresMessageRepository>>,
}
