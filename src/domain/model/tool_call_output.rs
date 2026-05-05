// src/domain/model/tool_call_output.rs

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

const TOOL_OUTPUT_TRUNCATED_MESSAGE: &str =
    "Tool call output was truncated because it exceeded the context limit.";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ToolCallOutputStatus {
    Success,
    Error,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolCallOutput {
    pub call_id: String,
    pub output: Value,
    pub status: ToolCallOutputStatus,
}

impl ToolCallOutput {
    pub fn success(call_id: impl Into<String>, output: Value) -> Self {
        Self {
            call_id: call_id.into(),
            output,
            status: ToolCallOutputStatus::Success,
        }
    }

    pub fn error(call_id: impl Into<String>, output: Value) -> Self {
        Self {
            call_id: call_id.into(),
            output,
            status: ToolCallOutputStatus::Error,
        }
    }

    pub fn is_success(&self) -> bool {
        self.status == ToolCallOutputStatus::Success
    }

    pub fn is_error(&self) -> bool {
        self.status == ToolCallOutputStatus::Error
    }

    /// Tool call output is bounded before it is saved back into the conversation context.
    pub fn truncate(self, max_output_chars: usize) -> Self {
        let serialized_output = match &self.output {
            Value::String(text) => text.clone(),
            value => serde_json::to_string(value).unwrap_or_else(|_| value.to_string()),
        };

        let original_chars = serialized_output.chars().count();
        if original_chars <= max_output_chars {
            return self;
        }

        let truncated_output = truncate_middle(&serialized_output, max_output_chars);

        Self {
            call_id: self.call_id,
            status: self.status,
            output: json!({
                "message": TOOL_OUTPUT_TRUNCATED_MESSAGE,
                "truncated": true,
                "original_chars": original_chars,
                "max_chars": max_output_chars,
                "output": truncated_output,
            }),
        }
    }
}

fn truncate_middle(text: &str, max_chars: usize) -> String {
    if max_chars == 0 {
        return String::new();
    }

    let marker = "\n\n[tool call output truncated]\n\n";
    let marker_chars = marker.chars().count();

    if max_chars <= marker_chars {
        return text.chars().take(max_chars).collect();
    }

    let available_chars = max_chars - marker_chars;
    let head_chars = available_chars * 2 / 3;
    let tail_chars = available_chars - head_chars;

    let head = text.chars().take(head_chars).collect::<String>();
    let tail = text
        .chars()
        .rev()
        .take(tail_chars)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<String>();

    format!("{head}{marker}{tail}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn short_output_is_not_truncated() {
        let output = ToolCallOutput::success("call-1", json!({ "message": "short" }));

        let truncated = output.clone().truncate(1000);

        assert_eq!(truncated, output);
    }

    #[test]
    fn long_output_is_truncated_with_metadata() {
        let output = ToolCallOutput::success("call-1", json!("abcdefghijklmnopqrstuvwxyz"));

        let truncated = output.truncate(10);

        assert_eq!(truncated.call_id, "call-1");
        assert_eq!(truncated.status, ToolCallOutputStatus::Success);
        assert_eq!(truncated.output["truncated"], json!(true));
        assert_eq!(truncated.output["max_chars"], json!(10));
    }
}
