use thiserror::Error;

#[derive(Debug, Error)]
pub enum ProcessManagerError {
    #[error("process not found: {0}")]
    NotFound(String),

    #[error("failed to start process: {0}")]
    StartFailed(String),

    #[error("failed to control process: {0}")]
    ControlFailed(String),
}
