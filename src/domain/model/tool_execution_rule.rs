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
}

impl std::str::FromStr for ToolExecutionRuleAction {
    type Err = ();

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "allow" => Ok(Self::Allow),
            "ask" => Ok(Self::Ask),
            "deny" => Ok(Self::Deny),
            _ => Err(()),
        }
    }
}
