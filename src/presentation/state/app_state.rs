use std::sync::Arc;

use crate::application::usecase::message_usecase::MessageUsecase;
use crate::application::usecase::session_usecase::SessionUsecase;
use crate::infrastructure::persistence::postgres_message_repository::PostgresMessageRepository;
use crate::infrastructure::persistence::postgres_session_repository::PostgresSessionRepository;

#[derive(Clone)]
pub struct AppState {
    pub session_usecase: Arc<SessionUsecase<PostgresSessionRepository>>,
    pub message_usecase: Arc<MessageUsecase<PostgresMessageRepository, PostgresSessionRepository>>,
}
