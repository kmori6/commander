use thiserror::Error;

#[derive(Debug, Error)]
pub enum SubagentRepositoryError {
    #[error("invalid subagent config: {0}")]
    InvalidConfig(String),

    #[error("failed to access subagent repository: {0}")]
    Unexpected(String),
}
