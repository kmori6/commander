use thiserror::Error;

#[derive(Debug, Error)]
pub enum ToolPermissionRepositoryError {
    #[error("invalid tool permission: {0}")]
    InvalidPermission(String),

    #[error("failed to access tool permission repository: {0}")]
    Unexpected(String),
}
