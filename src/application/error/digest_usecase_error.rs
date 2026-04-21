use thiserror::Error;

#[derive(Debug, Error)]
pub enum DigestUsecaseError {
    #[error("failed to fetch papers: {0}")]
    Fetch(String),

    #[error("failed to translate report: {0}")]
    Translate(String),
}
