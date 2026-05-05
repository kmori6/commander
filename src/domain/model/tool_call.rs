use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolSpec {
    pub name: String,
    pub description: String,
    pub parameters: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolCall {
    pub call_id: String,
    pub name: String,
    pub arguments: Value,
}

impl ToolCall {
    /// A tool call signature identifies repeated calls by tool name and arguments.
    pub fn signature(&self) -> String {
        serde_json::json!({
            "name": self.name,
            "arguments": self.arguments,
        })
        .to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn same_tool_name_and_arguments_have_same_signature() {
        let a = ToolCall {
            call_id: "call-1".to_string(),
            name: "shell_exec".to_string(),
            arguments: json!({ "command": "ls" }),
        };
        let b = ToolCall {
            call_id: "call-2".to_string(),
            name: "shell_exec".to_string(),
            arguments: json!({ "command": "ls" }),
        };

        assert_eq!(a.signature(), b.signature());
    }

    #[test]
    fn different_arguments_have_different_signatures() {
        let a = ToolCall {
            call_id: "call-1".to_string(),
            name: "shell_exec".to_string(),
            arguments: json!({ "command": "ls" }),
        };
        let b = ToolCall {
            call_id: "call-2".to_string(),
            name: "shell_exec".to_string(),
            arguments: json!({ "command": "pwd" }),
        };

        assert_ne!(a.signature(), b.signature());
    }
}
