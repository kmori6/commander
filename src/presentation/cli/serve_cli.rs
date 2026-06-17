use crate::application::config::CommanderPaths;
use crate::application::runtime::agent_runtime::AgentRuntime;
use crate::application::service::event_service::EventService;
use crate::application::service::instruction_service::InstructionService;
use crate::application::service::tool_service::ToolService;
use crate::application::usecase::message_usecase::MessageUsecase;
use crate::application::usecase::schedule_usecase::ScheduleUsecase;
use crate::application::usecase::session_usecase::SessionUsecase;
use crate::application::usecase::task_usecase::TaskUsecase;
use crate::application::usecase::tool_usecase::ToolUsecase;
use crate::domain::port::tool::Tool;
use crate::infrastructure::llm::bedrock_llm_provider::BedrockLlmProvider;
use crate::infrastructure::llm::llm_gateway::LlmGateway;
use crate::infrastructure::persistence::file_schedule_repository::FileScheduleRepository;
use crate::infrastructure::persistence::file_subagent_repository::FileSubagentRepository;
use crate::infrastructure::persistence::file_watch_repository::FileWatchRepository;
use crate::infrastructure::persistence::postgres_message_repository::PostgresMessageRepository;
use crate::infrastructure::persistence::postgres_session_repository::PostgresSessionRepository;
use crate::infrastructure::persistence::postgres_task_repository::PostgresTaskRepository;
use crate::infrastructure::tool::file_edit_tool::FileEditTool;
use crate::infrastructure::tool::file_list_tool::FileListTool;
use crate::infrastructure::tool::file_read_tool::FileReadTool;
use crate::infrastructure::tool::file_search_tool::FileSearchTool;
use crate::infrastructure::tool::file_write_tool::FileWriteTool;
use crate::infrastructure::tool::mcp_tool::load_mcp_tools;
use crate::infrastructure::tool::memory_write_tool::MemoryWriteTool;
use crate::infrastructure::tool::pptx_read_tool::PptxReadTool;
use crate::infrastructure::tool::shell_tool::ShellTool;
use crate::infrastructure::tool::text_search_tool::TextSearchTool;
use crate::infrastructure::tool::transcribe_tool::TranscribeTool;
use crate::infrastructure::tool::visual_inspect_tool::VisualInspectTool;
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
use crate::presentation::handler::get_task_usage_handler::get_task_usage_handler;
use crate::presentation::handler::health_handler::health_handler;
use crate::presentation::handler::list_message_handler::list_message_handler;
use crate::presentation::handler::list_model_handler::list_model_handler;
use crate::presentation::handler::list_schedule_handler::list_schedule_handler;
use crate::presentation::handler::list_schedule_run_handler::list_schedule_run_handler;
use crate::presentation::handler::list_session_handler::list_session_handler;
use crate::presentation::handler::list_task_handler::list_task_handler;
use crate::presentation::handler::list_tool_handler::list_tool_handler;
use crate::presentation::handler::run_schedule_handler::run_schedule_handler;
use crate::presentation::handler::run_watch_handler::run_watch_handler;
use crate::presentation::handler::update_model_handler::update_model_handler;
use crate::presentation::handler::update_schedule_handler::update_schedule_handler;
use crate::presentation::handler::update_session_handler::update_session_handler;
use crate::presentation::state::app_state::AppState;
use crate::presentation::worker::schedule_daemon::ScheduleDaemon;
use crate::presentation::worker::task_runner::TaskRunner;
use axum::Router;
use axum::routing::{get, post};
use sqlx::PgPool;
use std::{env, net::SocketAddr, sync::Arc, time::Duration};

pub async fn run(addr: SocketAddr) -> Result<(), std::io::Error> {
    // env
    let database_url = env::var("DATABASE_URL")
        .map_err(|err| std::io::Error::new(std::io::ErrorKind::NotFound, err))?;
    let sandbox_image = env::var("SANDBOX_IMAGE")
        .map_err(|err| std::io::Error::new(std::io::ErrorKind::NotFound, err))?;
    let paths = CommanderPaths::resolve().map_err(std::io::Error::other)?;
    paths.ensure_dirs().await.map_err(std::io::Error::other)?;
    let workspace_root = paths.workspace_path().to_path_buf();

    let pool = PgPool::connect(&database_url)
        .await
        .map_err(std::io::Error::other)?;

    // repositories
    let session_repository = PostgresSessionRepository::new(pool.clone());
    let task_repository = PostgresTaskRepository::new(pool.clone());
    let schedule_repository = FileScheduleRepository::new(paths.schedules_path());
    let message_repository = PostgresMessageRepository::new(pool.clone());
    let watch_repository = FileWatchRepository::new(paths.watch_config_path());
    let subagent_repository = FileSubagentRepository::new(workspace_root.join("subagents"));

    let visual_inspect_provider = BedrockLlmProvider::from_default_config().await;

    // services
    let llm_gateway = LlmGateway::from_config_path(paths.model_config_path())
        .await
        .map_err(std::io::Error::other)?;
    let event_service = Arc::new(EventService::new());
    let instruction_service = Arc::new(InstructionService::new(workspace_root.clone()));
    let mut tools: Vec<Arc<dyn Tool>> = vec![
        Arc::new(FileReadTool::new(workspace_root.clone())),
        Arc::new(PptxReadTool::new(workspace_root.clone())),
        Arc::new(FileWriteTool::new(workspace_root.clone())),
        Arc::new(FileEditTool::new(workspace_root.clone())),
        Arc::new(FileListTool::new(workspace_root.clone())),
        Arc::new(FileSearchTool::new(workspace_root.clone())),
        Arc::new(TextSearchTool::new(workspace_root.clone())),
        Arc::new(TranscribeTool::new(workspace_root.clone()).map_err(std::io::Error::other)?),
        Arc::new(ShellTool::new(
            workspace_root.clone(),
            paths.sandbox_env_path(),
            sandbox_image,
        )),
        Arc::new(WebSearchTool::from_env().map_err(std::io::Error::other)?),
        Arc::new(WebFetchTool::new().map_err(std::io::Error::other)?),
        Arc::new(VisualInspectTool::new(
            workspace_root.clone(),
            visual_inspect_provider,
        )),
        Arc::new(MemoryWriteTool::new(workspace_root.clone()).map_err(std::io::Error::other)?),
    ];

    // append MCP tools
    tools.extend(
        load_mcp_tools(paths.mcp_config_path())
            .await
            .map_err(std::io::Error::other)?,
    );

    let tool_service = Arc::new(ToolService::new(tools));

    // usecases
    let session_usecase = Arc::new(SessionUsecase::new(session_repository.clone()));
    let message_usecase = Arc::new(MessageUsecase::new(
        message_repository.clone(),
        session_repository.clone(),
        task_repository.clone(),
    ));
    let task_usecase = Arc::new(TaskUsecase::new(
        task_repository.clone(),
        message_repository.clone(),
    ));
    let tool_usecase = Arc::new(ToolUsecase::new(tool_service.clone()));
    let schedule_usecase = Arc::new(ScheduleUsecase::new(
        schedule_repository,
        task_repository.clone(),
        message_repository.clone(),
    ));
    let agent_runtime = Arc::new(AgentRuntime::new(
        llm_gateway,
        tool_service.clone(),
        task_repository.clone(),
        message_repository.clone(),
        subagent_repository,
        event_service.clone(),
        instruction_service.clone(),
    ));

    // workers
    let task_runner = TaskRunner::new(
        task_usecase.clone(),
        agent_runtime.clone(),
        Duration::from_secs(1),
    );
    let schedule_daemon = ScheduleDaemon::new(
        schedule_usecase.clone(),
        watch_repository,
        instruction_service.clone(),
    );

    // app state
    let app_state = AppState {
        session_usecase,
        message_usecase,
        task_usecase,
        agent_runtime,
        event_service,
        instruction_service,
        tool_usecase,
        schedule_usecase,
    };

    tokio::spawn(async move {
        schedule_daemon.run().await;
    });

    tokio::spawn(async move {
        task_runner.run().await;
    });

    let app = build_router(app_state);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await
}

pub fn build_router(app_state: AppState) -> Router {
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
        .route("/watch/run", post(run_watch_handler))
        .route("/tools", get(list_tool_handler))
        .route("/models", get(list_model_handler))
        .route("/model", get(get_model_handler).put(update_model_handler))
        .with_state(app_state);

    Router::new().nest("/v1", api_routes)
}
