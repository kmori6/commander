use thiserror::Error;

#[derive(Debug, Error)]
pub enum MessageError {
    #[error("message content must not be empty")]
    EmptyContents,

    #[error("message content must not mix message and tool content")]
    MixedContentTypes,
}
