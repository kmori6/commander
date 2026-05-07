use std::sync::Arc;

use crate::application::usecase::message_usecase::MessageUsecase;
use crate::application::usecase::session_usecase::SessionUsecase;
use crate::application::usecase::task_usecase::TaskUsecase;
use crate::infrastructure::persistence::postgres_message_repository::PostgresMessageRepository;
use crate::infrastructure::persistence::postgres_session_repository::PostgresSessionRepository;
use crate::infrastructure::persistence::postgres_task_repository::PostgresTaskRepository;

#[derive(Clone)]
pub struct AppState {
    pub session_usecase: Arc<SessionUsecase<PostgresSessionRepository>>,
    pub message_usecase: Arc<MessageUsecase<PostgresMessageRepository, PostgresSessionRepository>>,
    pub task_usecase: Arc<TaskUsecase<PostgresTaskRepository, PostgresSessionRepository>>,
}
