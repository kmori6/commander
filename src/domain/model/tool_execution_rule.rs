#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolExecutionRule {
    pub tool_name: String,
    pub action: ToolExecutionRuleAction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolExecutionRuleAction {
    Allow,
    Ask,
    Deny,
}

impl ToolExecutionRuleAction {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Allow => "allow",
            Self::Ask => "ask",
            Self::Deny => "deny",
        }
    }

    pub fn from_str(value: &str) -> Option<Self> {
        match value {
            "allow" => Some(Self::Allow),
            "ask" => Some(Self::Ask),
            "deny" => Some(Self::Deny),
            _ => None,
        }
    }
}
