use crate::domain::error::event_repository_error::EventRepositoryError;
use crate::domain::error::llm_provider_error::LlmProviderError;
use crate::domain::error::message_repository_error::MessageRepositoryError;
use crate::domain::error::task_repository_error::TaskRepositoryError;
use crate::domain::error::task_result_repository_error::TaskResultRepositoryError;
use crate::domain::error::token_usage_repository_error::TokenUsageRepositoryError;
use crate::domain::error::tool_approval_repository_error::ToolApprovalRepositoryError;
use crate::domain::error::tool_permission_repository_error::ToolPermissionRepositoryError;
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum AgentRuntimeError {
    #[error("failed to access task repository: {0}")]
    TaskRepository(#[from] TaskRepositoryError),

    #[error("failed to access task result repository: {0}")]
    TaskResultRepository(#[from] TaskResultRepositoryError),

    #[error("failed to access event repository: {0}")]
    EventRepository(#[from] EventRepositoryError),

    #[error("task not found")]
    TaskNotFound,

    #[error("failed to access message repository: {0}")]
    MessageRepository(#[from] MessageRepositoryError),

    #[error("failed to access token usage repository: {0}")]
    TokenUsageRepository(#[from] TokenUsageRepositoryError),

    #[error("failed to access LLM provider: {0}")]
    LlmProvider(#[from] LlmProviderError),

    #[error("unsupported agent runtime operation: {0}")]
    Unsupported(String),

    #[error("failed to access tool permission repository: {0}")]
    ToolPermissionRepository(#[from] ToolPermissionRepositoryError),

    #[error("failed to access tool approval repository: {0}")]
    ToolApprovalRepository(#[from] ToolApprovalRepositoryError),

    #[error("tool approval not found")]
    ToolApprovalNotFound,

    #[error("message not found: {0}")]
    MessageNotFound(Uuid),

    #[error("tool call not found: {0}")]
    ToolCallNotFound(String),

    #[error("tool approval is still pending: {0}")]
    ToolApprovalPending(Uuid),
}
