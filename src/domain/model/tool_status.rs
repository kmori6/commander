use crate::domain::model::tool_execution_decision::ToolExecutionDecision;
use crate::domain::model::tool_execution_policy::ToolExecutionPolicy;
use crate::domain::model::tool_execution_rule::ToolExecutionRuleAction;

#[derive(Debug, Clone)]
pub struct ToolStatus {
    pub tool_name: String,
    pub policy: ToolExecutionPolicy,
    pub rule: Option<ToolExecutionRuleAction>,
    pub action: ToolExecutionDecision,
    pub source: ToolStatusSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolStatusSource {
    Saved,
    Default,
}

impl ToolStatusSource {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Saved => "saved",
            Self::Default => "default",
        }
    }
}
