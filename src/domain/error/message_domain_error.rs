use thiserror::Error;

use crate::domain::model::message::Role;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum MessageDomainError {
    #[error("message contents must not be empty")]
    EmptyContents,

    #[error("invalid message content: {0}")]
    InvalidContent(String),

    #[error("runtime-only message content cannot be persisted")]
    RuntimeOnlyContent,

    #[error("message content does not fit role: {role:?}")]
    ContentRoleMismatch { role: Role },

    #[error("assistant response model must not be empty")]
    EmptyModel,

    #[error("assistant response requires model")]
    MissingModel,

    #[error("assistant response requires usage")]
    MissingUsage,

    #[error("only assistant responses may have model or usage")]
    UnexpectedResponseMetadata,

    #[error("usage token counts must be greater than or equal to zero")]
    InvalidUsage,
}
