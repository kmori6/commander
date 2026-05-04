use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum ChatSessionError {
    #[error("session is already running: {session_id}")]
    AlreadyRunning { session_id: Uuid },

    #[error("tool approval is pending: {session_id}")]
    ApprovalPending { session_id: Uuid },

    #[error("tool approval is not pending: {session_id}")]
    ApprovalNotPending { session_id: Uuid },

    #[error("session turn is not running: {session_id}")]
    TurnNotRunning { session_id: Uuid },
}
