use crate::application::error::tool_usecase_error::ToolUsecaseError;
use crate::domain::model::tool_execution_rule::{ToolExecutionRule, ToolExecutionRuleAction};
use crate::domain::model::tool_status::ToolStatus;
use crate::domain::repository::tool_execution_rule_repository::ToolExecutionRuleRepository;
use crate::domain::service::tool_service::ToolService;

pub struct ToolUsecase<R> {
    tool_service: ToolService,
    rule_repository: R,
}

impl<R> ToolUsecase<R>
where
    R: ToolExecutionRuleRepository,
{
    pub fn new(tool_service: ToolService, rule_repository: R) -> Self {
        Self {
            tool_service,
            rule_repository,
        }
    }

    pub async fn statuses(&self) -> Result<Vec<ToolStatus>, ToolUsecaseError> {
        Ok(self.tool_service.tool_statuses().await?)
    }

    pub async fn set_rule(
        &self,
        tool_name: String,
        action: ToolExecutionRuleAction,
    ) -> Result<ToolStatus, ToolUsecaseError> {
        if !self
            .tool_service
            .tool_names()
            .iter()
            .any(|name| name == &tool_name)
        {
            return Err(ToolUsecaseError::ToolNotFound(tool_name));
        }

        self.rule_repository
            .save(ToolExecutionRule {
                tool_name: tool_name.clone(),
                action,
            })
            .await?;

        self.statuses()
            .await?
            .into_iter()
            .find(|status| status.tool_name == tool_name)
            .ok_or(ToolUsecaseError::ToolNotFound(tool_name))
    }
}
