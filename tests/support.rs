use std::sync::Arc;

use axum::Router;
use commander::application::runtime::agent_runtime::AgentRuntime;
use commander::application::service::event_service::EventService;
use commander::application::service::instruction_service::InstructionService;
use commander::application::service::tool_service::ToolService;
use commander::application::usecase::message_usecase::MessageUsecase;
use commander::application::usecase::schedule_usecase::ScheduleUsecase;
use commander::application::usecase::session_usecase::SessionUsecase;
use commander::application::usecase::task_usecase::TaskUsecase;
use commander::infrastructure::llm::llm_gateway::LlmGateway;
use commander::infrastructure::persistence::file_schedule_repository::FileScheduleRepository;
use commander::infrastructure::persistence::file_subagent_repository::FileSubagentRepository;
use commander::infrastructure::persistence::postgres_message_repository::PostgresMessageRepository;
use commander::infrastructure::persistence::postgres_session_repository::PostgresSessionRepository;
use commander::infrastructure::persistence::postgres_task_repository::PostgresTaskRepository;
use commander::presentation::cli::serve_cli::build_router;
use commander::presentation::state::app_state::AppState;
use sqlx::PgPool;
use uuid::Uuid;

pub async fn test_app() -> Router {
    let pool = test_pool().await;
    let root = std::env::temp_dir().join(format!("commander-test-{}", Uuid::new_v4()));

    tokio::fs::create_dir_all(&root).await.unwrap();

    let session_repository = PostgresSessionRepository::new(pool.clone());
    let task_repository = PostgresTaskRepository::new(pool.clone());
    let message_repository = PostgresMessageRepository::new(pool.clone());

    let schedule_repository = FileScheduleRepository::new(root.join("schedules.json"));
    let subagent_repository = FileSubagentRepository::new(root.join("subagents"));

    let event_service = Arc::new(EventService::new());
    let instruction_service = Arc::new(InstructionService::new(root.clone()));
    let tool_service = Arc::new(ToolService::new(vec![]));

    let llm_gateway = LlmGateway::from_config_path(root.join("models.json"))
        .await
        .unwrap();

    let session_usecase = Arc::new(SessionUsecase::new(session_repository.clone()));
    let message_usecase = Arc::new(MessageUsecase::new(
        message_repository.clone(),
        session_repository,
        task_repository.clone(),
    ));
    let task_usecase = Arc::new(TaskUsecase::new(
        task_repository.clone(),
        message_repository.clone(),
    ));
    let schedule_usecase = Arc::new(ScheduleUsecase::new(
        schedule_repository,
        task_repository.clone(),
        message_repository.clone(),
    ));
    let agent_runtime = Arc::new(AgentRuntime::new(
        llm_gateway,
        tool_service.clone(),
        task_repository,
        message_repository,
        subagent_repository,
        event_service.clone(),
        instruction_service.clone(),
    ));

    build_router(AppState {
        event_service,
        instruction_service,
        session_usecase,
        message_usecase,
        task_usecase,
        schedule_usecase,
        tool_service,
        agent_runtime,
    })
}

async fn test_pool() -> PgPool {
    dotenvy::dotenv().ok();

    let database_url = std::env::var("TEST_DATABASE_URL").expect("TEST_DATABASE_URL is required");

    assert!(
        database_url.contains("commander_test"),
        "TEST_DATABASE_URL must point to commander_test"
    );

    let pool = PgPool::connect(&database_url).await.unwrap();

    sqlx::query("TRUNCATE messages, tasks, sessions CASCADE")
        .execute(&pool)
        .await
        .unwrap();

    pool
}
