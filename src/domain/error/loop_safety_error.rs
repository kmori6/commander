use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum LoopSafetyError {
    #[error("agent loop exceeded maximum llm steps: {max}")]
    MaxLlmStepsExceeded { max: usize },

    #[error("tool call failed repeatedly: {tool_name} repeated {repeats} times")]
    RepeatedFailedToolCall { tool_name: String, repeats: usize },
}
