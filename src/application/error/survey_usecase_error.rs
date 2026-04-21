use thiserror::Error;

#[derive(Debug, Error)]
pub enum SurveyUsecaseError {
    #[error("failed to read PDF: {0}")]
    PdfRead(String),

    #[error("failed to download PDF: {0}")] // ← 追加
    Download(String),

    #[error("LLM call failed: {0}")]
    LlmClient(String),
}
