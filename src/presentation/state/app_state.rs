use crate::infrastructure::persistence::postgres_chat_message_repository::PostgresChatMessageRepository;
use crate::infrastructure::persistence::postgres_chat_session_repository::PostgresChatSessionRepository;

#[derive(Clone)]
pub struct AppState {
    pub chat_session_repository: PostgresChatSessionRepository,
    pub chat_message_repository: PostgresChatMessageRepository,
}
