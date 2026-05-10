use crate::application::runtime::agent_runtime::AgentRuntime;
use crate::application::usecase::message_usecase::MessageUsecase;
use crate::application::usecase::schedule_usecase::ScheduleUsecase;
use crate::application::usecase::session_usecase::SessionUsecase;
use crate::application::usecase::task_usecase::TaskUsecase;
use crate::application::usecase::tool_approval_usecase::ToolApprovalUsecase;
use crate::application::usecase::tool_usecase::ToolUsecase;
use crate::domain::service::event_service::EventService;
use crate::domain::service::memory_index_service::MemoryIndexService;
use crate::domain::service::tool_executor::ToolExecutor;
use crate::infrastructure::embedding::bedrock_embedding_provider::BedrockEmbeddingProvider;
use crate::infrastructure::llm::llm_gateway::LlmGateway;
use crate::infrastructure::persistence::postgres_event_repository::PostgresEventRepository;
use crate::infrastructure::persistence::postgres_memory_index_repository::PostgresMemoryIndexRepository;
use crate::infrastructure::persistence::postgres_message_repository::PostgresMessageRepository;
use crate::infrastructure::persistence::postgres_schedule_repository::PostgresScheduleRepository;
use crate::infrastructure::persistence::postgres_session_repository::PostgresSessionRepository;
use crate::infrastructure::persistence::postgres_task_repository::PostgresTaskRepository;
use crate::infrastructure::persistence::postgres_task_result_repository::PostgresTaskResultRepository;
use crate::infrastructure::persistence::postgres_token_usage_repository::PostgresTokenUsageRepository;
use crate::infrastructure::persistence::postgres_tool_approval_repository::PostgresToolApprovalRepository;
use crate::infrastructure::persistence::postgres_tool_permission_repository::PostgresToolPermissionRepository;
use crate::infrastructure::tool::file_edit_tool::FileEditTool;
use crate::infrastructure::tool::file_list_tool::FileListTool;
use crate::infrastructure::tool::file_read_tool::FileReadTool;
use crate::infrastructure::tool::file_search_tool::FileSearchTool;
use crate::infrastructure::tool::file_write_tool::FileWriteTool;
use crate::infrastructure::tool::memory_search_tool::MemorySearchTool;
use crate::infrastructure::tool::memory_write_tool::MemoryWriteTool;
use crate::infrastructure::tool::shell_tool::ShellTool;
use crate::infrastructure::tool::text_search_tool::TextSearchTool;
use crate::infrastructure::tool::web_fetch_tool::WebFetchTool;
use crate::infrastructure::tool::web_search_tool::WebSearchTool;
use crate::presentation::handler::cancel_task_handler::cancel_task_handler;
use crate::presentation::handler::create_message_handler::create_message_handler;
use crate::presentation::handler::create_schedule_handler::create_schedule_handler;
use crate::presentation::handler::create_session_handler::create_session_handler;
use crate::presentation::handler::create_task_handler::create_task_handler;
use crate::presentation::handler::get_event_handler::get_event_handler;
use crate::presentation::handler::get_model_handler::get_model_handler;
use crate::presentation::handler::get_schedule_handler::get_schedule_handler;
use crate::presentation::handler::get_session_handler::get_session_handler;
use crate::presentation::handler::get_task_handler::get_task_handler;
use crate::presentation::handler::get_task_result_handler::get_task_result_handler;
use crate::presentation::handler::get_task_usage_handler::get_task_usage_handler;
use crate::presentation::handler::health_handler::health_handler;
use crate::presentation::handler::list_message_handler::list_message_handler;
use crate::presentation::handler::list_model_handler::list_model_handler;
use crate::presentation::handler::list_schedule_handler::list_schedule_handler;
use crate::presentation::handler::list_schedule_run_handler::list_schedule_run_handler;
use crate::presentation::handler::list_session_handler::list_session_handler;
use crate::presentation::handler::list_task_event_handler::list_task_event_handler;
use crate::presentation::handler::list_task_handler::list_task_handler;
use crate::presentation::handler::list_tool_approval_handler::list_tool_approval_handler;
use crate::presentation::handler::list_tool_handler::list_tool_handler;
use crate::presentation::handler::list_tool_permission_handler::list_tool_permission_handler;
use crate::presentation::handler::resolve_tool_approval_handler::{
    approve_tool_approval_handler, reject_tool_approval_handler,
};
use crate::presentation::handler::run_schedule_handler::run_schedule_handler;
use crate::presentation::handler::update_model_handler::update_model_handler;
use crate::presentation::handler::update_schedule_handler::update_schedule_handler;
use crate::presentation::handler::update_session_handler::update_session_handler;
use crate::presentation::handler::update_tool_permission_handler::update_tool_permission_handler;
use crate::presentation::state::app_state::AppState;
use axum::{
    Router,
    routing::{get, post, put},
};
use sqlx::PgPool;
use std::{env, net::SocketAddr, sync::Arc};

pub async fn run(addr: SocketAddr) -> Result<(), std::io::Error> {
    // env
    let database_url = env::var("DATABASE_URL")
        .map_err(|err| std::io::Error::new(std::io::ErrorKind::NotFound, err))?;
    let workspace_root = env::current_dir().map_err(std::io::Error::other)?;

    let pool = PgPool::connect(&database_url)
        .await
        .map_err(std::io::Error::other)?;

    // repositories
    let session_repository = PostgresSessionRepository::new(pool.clone());
    let task_repository = PostgresTaskRepository::new(pool.clone());
    let task_result_repository = PostgresTaskResultRepository::new(pool.clone());
    let event_repository = PostgresEventRepository::new(pool.clone());
    let tool_permission_repository = PostgresToolPermissionRepository::new(pool.clone());
    let schedule_repository = PostgresScheduleRepository::new(pool.clone());
    let tool_approval_repository = PostgresToolApprovalRepository::new(pool.clone());
    let message_repository = PostgresMessageRepository::new(pool.clone());
    let token_usage_repository = PostgresTokenUsageRepository::new(pool.clone());
    let memory_index_repository = Arc::new(PostgresMemoryIndexRepository::new(pool.clone()));

    // services
    let llm_gateway = LlmGateway::from_default_config()
        .await
        .map_err(std::io::Error::other)?;
    let embedding_provider = Arc::new(BedrockEmbeddingProvider::from_default_config().await);
    let memory_index_service = Arc::new(MemoryIndexService::new(
        embedding_provider,
        memory_index_repository,
    ));
    let event_service = Arc::new(EventService::new());
    let tool_executor = Arc::new(ToolExecutor::new(vec![
        Arc::new(FileReadTool::new(workspace_root.clone())),
        Arc::new(FileWriteTool::new(workspace_root.clone())),
        Arc::new(FileEditTool::new(workspace_root.clone())),
        Arc::new(FileListTool::new(workspace_root.clone())),
        Arc::new(FileSearchTool::new(workspace_root.clone())),
        Arc::new(TextSearchTool::new(workspace_root.clone())),
        Arc::new(ShellTool::new(workspace_root.clone())),
        Arc::new(WebSearchTool::from_env().map_err(std::io::Error::other)?),
        Arc::new(WebFetchTool::new().map_err(std::io::Error::other)?),
        Arc::new(MemorySearchTool::new(memory_index_service.clone())),
        Arc::new(
            MemoryWriteTool::new(workspace_root.clone(), memory_index_service.clone())
                .map_err(std::io::Error::other)?,
        ),
    ]));

    // usecases
    let session_usecase = Arc::new(SessionUsecase::new(session_repository.clone()));
    let message_usecase = Arc::new(MessageUsecase::new(
        message_repository.clone(),
        session_repository.clone(),
        task_repository.clone(),
    ));
    let task_usecase = Arc::new(TaskUsecase::new(
        task_repository.clone(),
        session_repository.clone(),
        task_result_repository.clone(),
        event_repository.clone(),
        token_usage_repository.clone(),
    ));
    let tool_usecase = Arc::new(ToolUsecase::new(
        tool_executor.clone(),
        tool_permission_repository.clone(),
    ));
    let schedule_usecase = Arc::new(ScheduleUsecase::new(
        schedule_repository,
        session_repository.clone(),
        task_repository.clone(),
    ));
    let tool_approval_usecase =
        Arc::new(ToolApprovalUsecase::new(tool_approval_repository.clone()));

    let model = llm_gateway.default_model_id().await;

    let agent_runtime = Arc::new(AgentRuntime::new(
        llm_gateway,
        tool_executor.clone(),
        task_repository.clone(),
        message_repository.clone(),
        task_result_repository.clone(),
        event_repository.clone(),
        token_usage_repository.clone(),
        event_service.clone(),
        tool_permission_repository.clone(),
        tool_approval_repository.clone(),
        model,
    ));

    // app state
    let app_state = AppState {
        session_usecase,
        message_usecase,
        task_usecase,
        agent_runtime,
        event_service,
        tool_usecase,
        schedule_usecase,
        tool_approval_usecase,
    };

    let api_routes = Router::new()
        .route("/health", get(health_handler))
        .route("/events", get(get_event_handler))
        .route(
            "/sessions",
            get(list_session_handler).post(create_session_handler),
        )
        .route(
            "/sessions/{id}",
            get(get_session_handler).patch(update_session_handler),
        )
        .route(
            "/sessions/{id}/messages",
            get(list_message_handler).post(create_message_handler),
        )
        .route("/tasks", get(list_task_handler).post(create_task_handler))
        .route("/tasks/{id}", get(get_task_handler))
        .route("/tasks/{id}/result", get(get_task_result_handler))
        .route("/tasks/{id}/events", get(list_task_event_handler))
        .route("/tasks/{id}/cancel", post(cancel_task_handler))
        .route("/tasks/{id}/usage", get(get_task_usage_handler))
        .route(
            "/schedules",
            get(list_schedule_handler).post(create_schedule_handler),
        )
        .route(
            "/schedules/{id}",
            get(get_schedule_handler).patch(update_schedule_handler),
        )
        .route("/schedules/{id}/run", post(run_schedule_handler))
        .route("/schedules/{id}/runs", get(list_schedule_run_handler))
        .route("/tools", get(list_tool_handler))
        .route("/tools/permissions", get(list_tool_permission_handler))
        .route(
            "/tools/permissions/{tool_name}",
            put(update_tool_permission_handler),
        )
        .route("/tools/approvals", get(list_tool_approval_handler))
        .route(
            "/tools/approvals/{id}/approve",
            post(approve_tool_approval_handler),
        )
        .route(
            "/tools/approvals/{id}/reject",
            post(reject_tool_approval_handler),
        )
        .route("/models", get(list_model_handler))
        .route("/model", get(get_model_handler).put(update_model_handler))
        .with_state(app_state);

    let app = Router::new().nest("/v1", api_routes);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await
}
