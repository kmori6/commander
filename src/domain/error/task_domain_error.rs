use thiserror::Error;

use crate::domain::model::task::TaskStatus;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum TaskDomainError {
    #[error("invalid task source: {0}")]
    InvalidSource(String),

    #[error("invalid task state: {0}")]
    InvalidState(String),

    #[error("invalid task transition: {from:?} -> {to:?}")]
    InvalidTransition { from: TaskStatus, to: TaskStatus },

    #[error("task is already terminal: {0:?}")]
    AlreadyTerminal(TaskStatus),

    #[error("task error must not be empty")]
    EmptyError,
}
