use crate::application::usecase::agent_usecase::AgentUsecase;
use crate::application::usecase::approval_usecase::ApprovalUsecase;
use crate::application::usecase::chat_session_usecase::ChatSessionUsecase;
use crate::application::usecase::job_execution_usecase::JobExecutionUsecase;
use crate::application::usecase::job_run_usecase::JobRunUsecase;
use crate::application::usecase::job_usecase::JobUsecase;
use crate::application::usecase::tool_usecase::ToolUsecase;
use crate::domain::service::event_service::EventService;
use crate::infrastructure::llm::bedrock_llm_provider::BedrockLlmProvider;
use crate::infrastructure::persistence::postgres_awaiting_tool_approval_repository::PostgresAwaitingToolApprovalRepository;
use crate::infrastructure::persistence::postgres_chat_message_repository::PostgresChatMessageRepository;
use crate::infrastructure::persistence::postgres_chat_session_repository::PostgresChatSessionRepository;
use crate::infrastructure::persistence::postgres_job_repository::PostgresJobRepository;
use crate::infrastructure::persistence::postgres_job_run_repository::PostgresJobRunRepository;
use crate::infrastructure::persistence::postgres_token_usage_repository::PostgresTokenUsageRepository;
use crate::infrastructure::persistence::postgres_tool_approval_repository::PostgresToolApprovalRepository;
use crate::infrastructure::persistence::postgres_tool_execution_rule_repository::PostgresToolExecutionRuleRepository;
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub chat_session_repository: PostgresChatSessionRepository,
    pub chat_message_repository: PostgresChatMessageRepository,
    pub token_usage_repository: PostgresTokenUsageRepository,
    pub chat_session_usecase:
        Arc<ChatSessionUsecase<PostgresChatSessionRepository, PostgresChatMessageRepository>>,
    pub tool_usecase: Arc<ToolUsecase<PostgresToolExecutionRuleRepository>>,
    pub job_usecase: Arc<JobUsecase<PostgresJobRepository>>,
    pub job_run_usecase: Arc<
        JobRunUsecase<
            PostgresJobRepository,
            PostgresJobRunRepository,
            PostgresChatMessageRepository,
        >,
    >,
    pub event_service: Arc<EventService>,
    pub agent_usecase: Arc<
        AgentUsecase<
            BedrockLlmProvider,
            PostgresChatSessionRepository,
            PostgresChatMessageRepository,
            PostgresTokenUsageRepository,
            PostgresToolApprovalRepository,
            PostgresAwaitingToolApprovalRepository,
        >,
    >,
    pub approval_usecase: Arc<ApprovalUsecase<PostgresAwaitingToolApprovalRepository>>,
    pub job_execution_usecase: Arc<
        JobExecutionUsecase<
            BedrockLlmProvider,
            PostgresJobRepository,
            PostgresJobRunRepository,
            PostgresChatSessionRepository,
            PostgresChatMessageRepository,
            PostgresTokenUsageRepository,
            PostgresToolApprovalRepository,
            PostgresAwaitingToolApprovalRepository,
        >,
    >,
}
