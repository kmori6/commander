use std::sync::Arc;

use crate::application::usecase::session_usecase::SessionUsecase;
use crate::infrastructure::persistence::postgres_session_repository::PostgresSessionRepository;

#[derive(Clone)]
pub struct AppState {
    pub session_usecase: Arc<SessionUsecase<PostgresSessionRepository>>,
}
