use thiserror::Error;

#[derive(Debug, Error)]
pub enum EmbeddingProviderError {
    #[error("embedding provider is unavailable: {0}")]
    Unavailable(String),

    #[error("failed to build embedding request: {0}")]
    RequestBuild(String),

    #[error("embedding API call failed: {0}")]
    ApiCall(String),

    #[error("failed to parse embedding response: {0}")]
    ResponseParse(String),
}
