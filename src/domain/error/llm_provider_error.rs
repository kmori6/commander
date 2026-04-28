use thiserror::Error;

#[derive(Debug, Error)]
pub enum LlmProviderError {
    #[error("Failed to build LLM request: {0}")]
    RequestBuild(String),

    #[error("LLM API call failed: {0}")]
    ApiCall(String),

    #[error("Failed to parse LLM response: {0}")]
    ResponseParse(String),
}
