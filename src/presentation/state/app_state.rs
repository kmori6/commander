use std::sync::Arc;

use crate::application::runtime::agent_runtime::AgentRuntime;
use crate::application::usecase::message_usecase::MessageUsecase;
use crate::application::usecase::schedule_usecase::ScheduleUsecase;
use crate::application::usecase::session_usecase::SessionUsecase;
use crate::application::usecase::task_usecase::TaskUsecase;
use crate::application::usecase::tool_approval_usecase::ToolApprovalUsecase;
use crate::application::usecase::tool_usecase::ToolUsecase;
use crate::domain::service::event_service::EventService;
use crate::infrastructure::llm::bedrock_llm_provider::BedrockLlmProvider;
use crate::infrastructure::persistence::postgres_event_repository::PostgresEventRepository;
use crate::infrastructure::persistence::postgres_message_repository::PostgresMessageRepository;
use crate::infrastructure::persistence::postgres_schedule_repository::PostgresScheduleRepository;
use crate::infrastructure::persistence::postgres_session_repository::PostgresSessionRepository;
use crate::infrastructure::persistence::postgres_task_repository::PostgresTaskRepository;
use crate::infrastructure::persistence::postgres_task_result_repository::PostgresTaskResultRepository;
use crate::infrastructure::persistence::postgres_token_usage_repository::PostgresTokenUsageRepository;
use crate::infrastructure::persistence::postgres_tool_approval_repository::PostgresToolApprovalRepository;
use crate::infrastructure::persistence::postgres_tool_permission_repository::PostgresToolPermissionRepository;
use crate::infrastructure::tool::mock_tool_executor::MockToolExecutor;

#[derive(Clone)]
pub struct AppState {
    // services
    pub event_service: Arc<EventService>,
    // usecases
    pub session_usecase: Arc<SessionUsecase<PostgresSessionRepository>>,
    pub message_usecase: Arc<
        MessageUsecase<
            PostgresMessageRepository,
            PostgresSessionRepository,
            PostgresTaskRepository,
        >,
    >,
    pub task_usecase: Arc<
        TaskUsecase<
            PostgresTaskRepository,
            PostgresSessionRepository,
            PostgresTaskResultRepository,
            PostgresEventRepository,
            PostgresTokenUsageRepository,
        >,
    >,
    pub schedule_usecase: Arc<
        ScheduleUsecase<
            PostgresScheduleRepository,
            PostgresSessionRepository,
            PostgresTaskRepository,
        >,
    >,
    pub tool_usecase: Arc<ToolUsecase<PostgresToolPermissionRepository>>,
    pub tool_approval_usecase: Arc<ToolApprovalUsecase<PostgresToolApprovalRepository>>,
    // runtimes
    pub agent_runtime: Arc<
        AgentRuntime<
            BedrockLlmProvider,
            MockToolExecutor,
            PostgresTaskRepository,
            PostgresMessageRepository,
            PostgresTaskResultRepository,
            PostgresEventRepository,
            PostgresTokenUsageRepository,
            PostgresToolPermissionRepository,
        >,
    >,
}
