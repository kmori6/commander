use thiserror::Error;

#[derive(Debug, Error)]
pub enum LlmProviderError {
    #[error("failed to build LLM request: {0}")]
    RequestBuild(String),

    #[error("failed to call LLM API: {0}")]
    ApiCall(String),

    #[error("failed to parse LLM response: {0}")]
    ResponseParse(String),
}
