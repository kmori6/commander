use thiserror::Error;

#[derive(Debug, Error)]
pub enum WatchRepositoryError {
    #[error("invalid watch config: {0}")]
    InvalidConfig(String),

    #[error("failed to access watch repository: {0}")]
    Unexpected(String),
}
