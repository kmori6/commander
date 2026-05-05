use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum LoopSafetyError {
    #[error("agent loop exceeded maximum llm steps: {max}")]
    MaxLlmStepsExceeded { max: usize },
}
