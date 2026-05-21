use async_trait::async_trait;

use crate::domain::error::subagent_repository_error::SubagentRepositoryError;
use crate::domain::model::subagent::Subagent;

#[async_trait]
pub trait SubagentRepository: Send + Sync {
    async fn list(&self) -> Result<Vec<Subagent>, SubagentRepositoryError>;
}
