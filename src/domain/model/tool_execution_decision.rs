use crate::domain::model::tool_execution_rule::ToolExecutionRuleAction;
use crate::domain::port::tool::ToolExecutionPolicy;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolExecutionDecision {
    Allow,
    Ask,
    Deny,
}

impl ToolExecutionDecision {
    pub fn decide(policy: ToolExecutionPolicy, rule: Option<ToolExecutionRuleAction>) -> Self {
        match (policy, rule) {
            (_, Some(ToolExecutionRuleAction::Deny)) => Self::Deny,
            (ToolExecutionPolicy::ConfirmEveryTime, _) => Self::Ask,
            (_, Some(ToolExecutionRuleAction::Allow)) => Self::Allow,
            (_, Some(ToolExecutionRuleAction::Ask)) => Self::Ask,
            (ToolExecutionPolicy::Auto, None) => Self::Allow,
            (ToolExecutionPolicy::Ask, None) => Self::Ask,
        }
    }
}
