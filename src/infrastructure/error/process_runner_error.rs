use thiserror::Error;

#[derive(Debug, Error)]
pub enum ProcessRunnerError {
    #[error("command timed out after {seconds} seconds")]
    TimedOut { seconds: u64 },

    #[error("failed to execute process: {0}")]
    ExecutionFailed(String),
}
