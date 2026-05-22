use thiserror::Error;

#[derive(Debug, Error)]
pub enum SubagentRepositoryError {
    #[error("failed to access subagent repository: {0}")]
    Unexpected(String),
}
