use async_trait::async_trait;

use crate::domain::error::watch_repository_error::WatchRepositoryError;
use crate::domain::model::watch::Watch;

#[async_trait]
pub trait WatchRepository: Send + Sync {
    async fn get(&self) -> Result<Option<Watch>, WatchRepositoryError>;
}
