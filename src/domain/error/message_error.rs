use thiserror::Error;

#[derive(Debug, Error)]
pub enum MessageError {
    #[error("message content must not be empty")]
    EmptyContents,

    #[error("invalid message content: {0}")]
    InvalidContent(String),
}
