use crate::application::usecase::message_usecase::MessageUsecase;
use crate::application::usecase::session_usecase::SessionUsecase;
use crate::infrastructure::persistence::postgres_message_repository::PostgresMessageRepository;
use crate::infrastructure::persistence::postgres_session_repository::PostgresSessionRepository;
use crate::presentation::handler::create_message_handler::create_message_handler;
use crate::presentation::handler::create_session_handler::create_session_handler;
use crate::presentation::handler::get_session_handler::get_session_handler;
use crate::presentation::handler::health_handler::health_handler;
use crate::presentation::handler::list_message_handler::list_message_handler;
use crate::presentation::handler::list_session_handler::list_session_handler;
use crate::presentation::handler::update_session_handler::update_session_handler;
use crate::presentation::state::app_state::AppState;
use axum::{Router, routing::get};
use sqlx::PgPool;
use std::{env, net::SocketAddr, sync::Arc};

pub async fn run(addr: SocketAddr) -> Result<(), std::io::Error> {
    let database_url = env::var("DATABASE_URL")
        .map_err(|err| std::io::Error::new(std::io::ErrorKind::NotFound, err))?;

    let pool = PgPool::connect(&database_url)
        .await
        .map_err(std::io::Error::other)?;

    let session_repository = PostgresSessionRepository::new(pool.clone());
    let message_repository = PostgresMessageRepository::new(pool);

    let session_usecase = Arc::new(SessionUsecase::new(session_repository.clone()));
    let message_usecase = Arc::new(MessageUsecase::new(message_repository, session_repository));

    let app_state = AppState {
        session_usecase,
        message_usecase,
    };

    let api_routes = Router::new()
        .route("/health", get(health_handler))
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
        .with_state(app_state);

    let app = Router::new().nest("/v1", api_routes);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await
}
