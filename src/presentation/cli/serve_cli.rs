use crate::application::usecase::agent_usecase::{AgentUsecase, AgentUsecaseRepositories};
use crate::application::usecase::job_usecase::JobUsecase;
use crate::application::usecase::tool_usecase::ToolUsecase;
use crate::domain::service::{
    agent_service::AgentService, compaction_service::CompactionService,
    event_service::EventService, instruction_service::InstructionService,
    memory_index_service::MemoryIndexService, tool_service::ToolService,
};
use crate::infrastructure::embedding::bedrock_embedding_provider::BedrockEmbeddingProvider;
use crate::infrastructure::llm::bedrock_llm_provider::BedrockLlmProvider;
use crate::infrastructure::persistence::postgres_awaiting_tool_approval_repository::PostgresAwaitingToolApprovalRepository;
use crate::infrastructure::persistence::postgres_chat_message_repository::PostgresChatMessageRepository;
use crate::infrastructure::persistence::postgres_chat_session_repository::PostgresChatSessionRepository;
use crate::infrastructure::persistence::postgres_job_repository::PostgresJobRepository;
use crate::infrastructure::persistence::postgres_memory_index_repository::PostgresMemoryIndexRepository;
use crate::infrastructure::persistence::postgres_token_usage_repository::PostgresTokenUsageRepository;
use crate::infrastructure::persistence::postgres_tool_approval_repository::PostgresToolApprovalRepository;
use crate::infrastructure::persistence::postgres_tool_execution_rule_repository::PostgresToolExecutionRuleRepository;
use crate::infrastructure::tool::{
    asr_tool::AsrTool, file_edit_tool::FileEditTool, file_read_tool::FileReadTool,
    file_search_tool::FileSearchTool, file_write_tool::FileWriteTool,
    memory_search_tool::MemorySearchTool, memory_write_tool::MemoryWriteTool, ocr_tool::OcrTool,
    shell_exec_tool::ShellExecTool, text_search_tool::TextSearchTool, web_fetch_tool::WebFetchTool,
    web_search_tool::WebSearchTool,
};
use crate::presentation::handler::create_event_handler::create_event_handler;
use crate::presentation::handler::create_message_handler::create_message_handler;
use crate::presentation::handler::create_session_handler::create_session_handler;
use crate::presentation::handler::delete_session_handler::delete_session_handler;
use crate::presentation::handler::get_job_handler::get_job_handler;
use crate::presentation::handler::get_session_handler::get_session_handler;
use crate::presentation::handler::get_session_usage_handler::get_session_usage_handler;
use crate::presentation::handler::health_handler::health_handler;
use crate::presentation::handler::list_approval_handler::list_approval_handler;
use crate::presentation::handler::list_job_handler::list_job_handler;
use crate::presentation::handler::list_message_handler::list_message_handler;
use crate::presentation::handler::list_session_handler::list_session_handler;
use crate::presentation::handler::list_tool_handler::list_tool_handler;
use crate::presentation::handler::resolve_approval_handler::resolve_approval_handler;
use crate::presentation::handler::update_tool_rule_handler::update_tool_rule_handler;
use crate::presentation::state::app_state::AppState;
use axum::{
    Router,
    routing::{get, post, put},
};
use sqlx::PgPool;
use std::{env, net::SocketAddr, sync::Arc};

pub async fn run(addr: SocketAddr) -> Result<(), std::io::Error> {
    let database_url = env::var("DATABASE_URL")
        .map_err(|err| std::io::Error::new(std::io::ErrorKind::NotFound, err))?;

    let pool = PgPool::connect(&database_url)
        .await
        .map_err(std::io::Error::other)?;

    let llm_client = BedrockLlmProvider::from_default_config().await;
    let embedding_provider = Arc::new(BedrockEmbeddingProvider::from_default_config().await);
    let memory_index_repository = Arc::new(PostgresMemoryIndexRepository::new(pool.clone()));
    let memory_index_service = Arc::new(MemoryIndexService::new(
        embedding_provider,
        memory_index_repository,
    ));
    let workspace_root = env::current_dir()?;
    let instruction_service = InstructionService::new(workspace_root.clone());

    let tool_execution_rule_repository = PostgresToolExecutionRuleRepository::new(pool.clone());
    let tool_service = ToolService::new(
        vec![
            Arc::new(AsrTool::from_env(workspace_root.clone()).map_err(std::io::Error::other)?),
            Arc::new(
                FileSearchTool::new(workspace_root.clone(), 200).map_err(std::io::Error::other)?,
            ),
            Arc::new(OcrTool::new(workspace_root.clone()).map_err(std::io::Error::other)?),
            Arc::new(ShellExecTool::new(workspace_root.clone()).map_err(std::io::Error::other)?),
            Arc::new(MemorySearchTool::new(memory_index_service.clone())),
            Arc::new(
                MemoryWriteTool::new(workspace_root.clone(), memory_index_service.clone())
                    .map_err(std::io::Error::other)?,
            ),
            Arc::new(FileWriteTool::new(workspace_root.clone()).map_err(std::io::Error::other)?),
            Arc::new(
                FileEditTool::new(workspace_root.clone(), 1_048_576)
                    .map_err(std::io::Error::other)?,
            ),
            Arc::new(
                FileReadTool::new(workspace_root.clone(), 1_048_576)
                    .map_err(std::io::Error::other)?,
            ),
            Arc::new(
                TextSearchTool::new(workspace_root.clone(), 1_048_576, 200, 10)
                    .map_err(std::io::Error::other)?,
            ),
            Arc::new(WebFetchTool::new().map_err(std::io::Error::other)?),
            Arc::new(WebSearchTool::from_env().map_err(std::io::Error::other)?),
        ],
        Arc::new(tool_execution_rule_repository.clone()),
    );

    let tool_usecase = Arc::new(ToolUsecase::new(
        tool_service.clone(),
        tool_execution_rule_repository.clone(),
    ));

    let compaction_service = CompactionService::new(llm_client.clone());
    let agent_service = AgentService::new(llm_client, tool_service);

    let chat_session_repository = PostgresChatSessionRepository::new(pool.clone());
    let chat_message_repository = PostgresChatMessageRepository::new(pool.clone());
    let token_usage_repository = PostgresTokenUsageRepository::new(pool.clone());
    let job_repository = PostgresJobRepository::new(pool.clone());
    let tool_approval_repository = PostgresToolApprovalRepository::new(pool.clone());
    let awaiting_tool_approval_repository =
        PostgresAwaitingToolApprovalRepository::new(pool.clone());
    let job_usecase = Arc::new(JobUsecase::new(job_repository));
    let agent_usecase = Arc::new(AgentUsecase::new(
        agent_service,
        instruction_service,
        compaction_service,
        AgentUsecaseRepositories {
            chat_session_repository: chat_session_repository.clone(),
            chat_message_repository: chat_message_repository.clone(),
            token_usage_repository: token_usage_repository.clone(),
            tool_approval_repository,
            awaiting_tool_approval_repository,
        },
    ));

    let app_state = AppState {
        chat_session_repository,
        chat_message_repository,
        token_usage_repository,
        tool_usecase,
        job_usecase,
        event_service: Arc::new(EventService::new()),
        agent_usecase,
    };

    let api_routes = Router::new()
        .route("/health", get(health_handler))
        .route("/events", get(create_event_handler))
        .route("/tools", get(list_tool_handler))
        .route("/tools/{tool_name}/rule", put(update_tool_rule_handler))
        .route("/approvals", get(list_approval_handler))
        .route("/jobs", get(list_job_handler))
        .route("/jobs/{id}", get(get_job_handler))
        .route(
            "/sessions",
            get(list_session_handler).post(create_session_handler),
        )
        .route(
            "/sessions/{id}",
            get(get_session_handler).delete(delete_session_handler),
        )
        .route(
            "/sessions/{id}/messages",
            get(list_message_handler).post(create_message_handler),
        )
        .route("/sessions/{id}/approvals", post(resolve_approval_handler))
        .route("/sessions/{id}/usage", get(get_session_usage_handler))
        .with_state(app_state);

    let app = Router::new().nest("/v1", api_routes);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await
}
