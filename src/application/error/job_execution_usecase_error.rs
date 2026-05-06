use crate::application::error::agent_runtime_error::AgentRuntimeError;
use crate::domain::error::chat_repository_error::ChatRepositoryError;
use crate::domain::error::chat_session_error::ChatSessionError;
use crate::domain::error::job_error::JobError;
use crate::domain::error::job_repository_error::JobRepositoryError;
use crate::domain::error::job_run_error::JobRunError;
use crate::domain::error::job_run_repository_error::JobRunRepositoryError;
use crate::domain::error::message_error::MessageError;
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum JobExecutionUsecaseError {
    #[error("job not found: {0}")]
    JobNotFound(Uuid),

    #[error("job run not found: {0}")]
    JobRunNotFound(Uuid),

    #[error("job run {run_id} does not belong to job {job_id}")]
    JobRunDoesNotBelongToJob { job_id: Uuid, run_id: Uuid },

    #[error("failed to access job repository: {0}")]
    JobRepository(#[from] JobRepositoryError),

    #[error("failed to access job run repository: {0}")]
    JobRunRepository(#[from] JobRunRepositoryError),

    #[error("invalid job operation: {0}")]
    Job(#[from] JobError),

    #[error("invalid job run operation: {0}")]
    JobRun(#[from] JobRunError),

    #[error("chat session not found: {0}")]
    ChatSessionNotFound(Uuid),

    #[error("failed to access chat repository: {0}")]
    ChatRepository(#[from] ChatRepositoryError),

    #[error("invalid chat session state: {0}")]
    ChatSession(#[from] ChatSessionError),

    #[error("invalid message: {0}")]
    Message(#[from] MessageError),

    #[error("failed to run agent runtime: {0}")]
    AgentRuntime(#[from] AgentRuntimeError),
}
