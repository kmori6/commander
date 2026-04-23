use std::collections::HashMap;

use crate::domain::model::tool_execution_decision::ToolExecutionDecision;
use crate::domain::model::tool_execution_rule::{ToolExecutionRule, ToolExecutionRuleAction};
use crate::domain::port::tool::ToolExecutionPolicy;

#[derive(Debug, Clone, Default)]
pub struct ToolExecutionRules {
    actions: HashMap<String, ToolExecutionRuleAction>,
}

impl ToolExecutionRules {
    pub fn from_rules(rules: Vec<ToolExecutionRule>) -> Self {
        Self {
            actions: rules
                .into_iter()
                .map(|rule| (rule.tool_name, rule.action))
                .collect(),
        }
    }

    pub fn action_for(&self, tool_name: &str) -> Option<ToolExecutionRuleAction> {
        self.actions.get(tool_name).copied()
    }

    pub fn decide(&self, tool_name: &str, policy: ToolExecutionPolicy) -> ToolExecutionDecision {
        ToolExecutionDecision::decide(policy, self.action_for(tool_name))
    }
}
