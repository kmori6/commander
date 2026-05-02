use thiserror::Error;

#[derive(Debug, Error)]
pub enum MessageError {
    #[error("message content must not be empty")]
    EmptyContents,
}
