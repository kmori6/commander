use crate::domain::error::llm_provider_error::LlmProviderError;
use crate::domain::error::message_repository_error::MessageRepositoryError;
use crate::domain::error::subagent_repository_error::SubagentRepositoryError;
use crate::domain::error::task_repository_error::TaskRepositoryError;
use crate::domain::error::tool_approval_repository_error::ToolApprovalRepositoryError;
use crate::domain::error::tool_permission_repository_error::ToolPermissionRepositoryError;
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum AgentRuntimeError {
    #[error("failed to access task repository: {0}")]
    TaskRepository(#[from] TaskRepositoryError),

    #[error("task not found")]
    TaskNotFound,

    #[error("task is already running: {0}")]
    TaskAlreadyRunning(Uuid),

    #[error("failed to access message repository: {0}")]
    MessageRepository(#[from] MessageRepositoryError),

    #[error("failed to access subagent repository: {0}")]
    SubagentRepository(#[from] SubagentRepositoryError),

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

    #[error("failed to access runtime file: {0}")]
    Io(#[from] std::io::Error),
}
