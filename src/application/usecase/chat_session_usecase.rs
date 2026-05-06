use crate::application::error::chat_session_usecase_error::ChatSessionUsecaseError;
use crate::domain::model::chat_session::{ChatSession, ChatSessionStatus};
use crate::domain::repository::chat_message_repository::ChatMessageRepository;
use crate::domain::repository::chat_session_repository::ChatSessionRepository;
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use uuid::Uuid;

const UNTITLED_SESSION_TITLE: &str = "Untitled session";

#[derive(Debug, Clone)]
pub struct ChatSessionListItem {
    pub id: Uuid,
    pub title: String,
    pub status: ChatSessionStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub message_count: i64,
}

pub struct ChatSessionUsecase<S, M> {
    chat_session_repository: S,
    chat_message_repository: M,
}

impl<S, M> ChatSessionUsecase<S, M>
where
    S: ChatSessionRepository,
    M: ChatMessageRepository,
{
    pub fn new(chat_session_repository: S, chat_message_repository: M) -> Self {
        Self {
            chat_session_repository,
            chat_message_repository,
        }
    }

    pub async fn start(&self) -> Result<ChatSession, ChatSessionUsecaseError> {
        self.chat_session_repository
            .create()
            .await
            .map_err(Into::into)
    }

    pub async fn find(
        &self,
        session_id: Uuid,
    ) -> Result<Option<ChatSession>, ChatSessionUsecaseError> {
        self.chat_session_repository
            .find_by_id(session_id)
            .await
            .map_err(Into::into)
    }

    pub async fn list(
        &self,
        limit: usize,
    ) -> Result<Vec<ChatSessionListItem>, ChatSessionUsecaseError> {
        let sessions = self.chat_session_repository.list_recent(limit).await?;
        let session_ids = sessions
            .iter()
            .map(|session| session.id)
            .collect::<Vec<_>>();

        let message_summaries = self
            .chat_message_repository
            .summarize_by_session_ids(&session_ids)
            .await?
            .into_iter()
            .map(|summary| (summary.session_id, summary))
            .collect::<HashMap<_, _>>();

        Ok(sessions
            .into_iter()
            .map(|session| {
                let message_summary = message_summaries.get(&session.id);
                let title = session
                    .title
                    .clone()
                    .filter(|title| !title.trim().is_empty())
                    .or_else(|| {
                        message_summary
                            .and_then(|summary| summary.first_user_message.as_deref())
                            .and_then(ChatSession::title_from_first_user_message)
                    })
                    .unwrap_or_else(|| UNTITLED_SESSION_TITLE.to_string());

                ChatSessionListItem {
                    id: session.id,
                    title,
                    status: session.status,
                    created_at: session.created_at,
                    updated_at: session.updated_at,
                    message_count: message_summary
                        .map(|summary| summary.message_count)
                        .unwrap_or(0),
                }
            })
            .collect())
    }
}
