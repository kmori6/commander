use crate::domain::error::llm_provider_error::LlmProviderError;
use crate::domain::port::search_provider::SearchError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DeepResearchError {
    #[error("llm request failed: {0}")]
    LlmProvider(#[from] LlmProviderError),

    #[error("search request failed: {0}")]
    Search(#[from] SearchError),
}
