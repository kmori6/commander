use crate::domain::error::tool_error::ToolError;
use crate::domain::model::tool::{ToolExecutionResult, ToolSpec};
use async_trait::async_trait;
use serde_json::Value;

/// A tool's default execution policy for a specific invocation.
///
/// This is declared by the tool implementation and then combined with
/// persisted `tool_execution_rules` to produce the final execution decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolExecutionPolicy {
    /// Execute automatically unless a persisted rule asks or denies it.
    Auto,
    /// Ask the user before execution unless a persisted rule allows or denies it.
    Ask,
    /// Always ask before execution unless a persisted rule denies it.
    ///
    /// A stored `allow` rule must not bypass this policy.
    ConfirmEveryTime,
}

#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn parameters(&self) -> Value;

    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: self.name().to_string(),
            description: self.description().to_string(),
            parameters: self.parameters(),
        }
    }

    /// Returns the tool's default execution policy for this invocation.
    ///
    /// Implementations may inspect `arguments` to choose a stricter policy for
    /// risky operations. Read-only tools can usually keep the default `Auto`.
    fn execution_policy(&self, _arguments: &Value) -> ToolExecutionPolicy {
        ToolExecutionPolicy::Auto
    }

    async fn execute(&self, arguments: Value) -> Result<ToolExecutionResult, ToolError>;
}
