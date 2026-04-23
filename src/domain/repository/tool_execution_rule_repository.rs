use crate::domain::error::tool_execution_rule_repository_error::ToolExecutionRuleRepositoryError;
use crate::domain::model::tool_execution_rule::ToolExecutionRule;
use async_trait::async_trait;

#[async_trait]
pub trait ToolExecutionRuleRepository: Send + Sync {
    async fn find_by_tool_name(
        &self,
        tool_name: &str,
    ) -> Result<Option<ToolExecutionRule>, ToolExecutionRuleRepositoryError>;

    async fn save(&self, rule: ToolExecutionRule) -> Result<(), ToolExecutionRuleRepositoryError>;

    async fn list_all(&self) -> Result<Vec<ToolExecutionRule>, ToolExecutionRuleRepositoryError>;
}
