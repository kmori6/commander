use crate::domain::error::loop_safety_error::LoopSafetyError;
use crate::domain::model::tool_call::{ToolCall, ToolCallOutput, ToolCallOutputStatus};
use serde_json::{Value, json};

const TOOL_OUTPUT_TRUNCATED_MESSAGE: &str =
    "Tool call output was truncated because it exceeded the loop safety limit.";
const DEFAULT_MAX_FAILED_REPEATS: usize = 5;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoopSafety {
    max_llm_steps: usize,
    llm_steps: usize,
    max_failed_repeats: usize,
    last_failed_signature: Option<String>,
    failed_repeats: usize,
}

impl LoopSafety {
    pub fn new(max_llm_steps: usize) -> Self {
        Self {
            max_llm_steps,
            llm_steps: 0,
            max_failed_repeats: DEFAULT_MAX_FAILED_REPEATS,
            last_failed_signature: None,
            failed_repeats: 0,
        }
    }

    pub fn llm_steps(&self) -> usize {
        self.llm_steps
    }

    pub fn max_llm_steps(&self) -> usize {
        self.max_llm_steps
    }

    /// An agent turn may start only a bounded number of LLM steps.
    pub fn start_llm_step(&mut self) -> Result<(), LoopSafetyError> {
        if self.llm_steps >= self.max_llm_steps {
            return Err(LoopSafetyError::MaxLlmStepsExceeded {
                max: self.max_llm_steps,
            });
        }

        self.llm_steps += 1;
        Ok(())
    }

    /// Tool call output is bounded before it is saved back into the conversation context.
    pub fn truncate_tool_call_output(
        output: ToolCallOutput,
        max_output_chars: usize,
    ) -> ToolCallOutput {
        let serialized_output = match &output.output {
            Value::String(text) => text.clone(),
            value => serde_json::to_string(value).unwrap_or_else(|_| value.to_string()),
        };

        let original_chars = serialized_output.chars().count();
        if original_chars <= max_output_chars {
            return output;
        }

        let truncated_output = truncate_middle(&serialized_output, max_output_chars);

        ToolCallOutput {
            call_id: output.call_id,
            status: output.status,
            output: json!({
                "message": TOOL_OUTPUT_TRUNCATED_MESSAGE,
                "truncated": true,
                "original_chars": original_chars,
                "max_chars": max_output_chars,
                "output": truncated_output,
            }),
        }
    }

    /// A repeated failed tool call must not continue indefinitely within one agent turn.
    pub fn record_tool_call_output(
        &mut self,
        tool_call: &ToolCall,
        output: &ToolCallOutput,
    ) -> Result<(), LoopSafetyError> {
        if output.status == ToolCallOutputStatus::Success {
            self.last_failed_signature = None;
            self.failed_repeats = 0;
            return Ok(());
        }

        let signature = tool_call.signature();

        if self.last_failed_signature.as_deref() == Some(signature.as_str()) {
            self.failed_repeats += 1;
        } else {
            self.last_failed_signature = Some(signature);
            self.failed_repeats = 1;
        }

        if self.failed_repeats >= self.max_failed_repeats {
            return Err(LoopSafetyError::RepeatedFailedToolCall {
                tool_name: tool_call.name.clone(),
                repeats: self.failed_repeats,
            });
        }

        Ok(())
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

    #[test]
    fn allows_steps_up_to_the_configured_limit() {
        let mut safety = LoopSafety::new(2);

        assert_eq!(safety.start_llm_step(), Ok(()));
        assert_eq!(safety.start_llm_step(), Ok(()));
        assert_eq!(safety.llm_steps(), 2);
    }

    #[test]
    fn rejects_steps_after_the_configured_limit() {
        let mut safety = LoopSafety::new(1);

        assert_eq!(safety.start_llm_step(), Ok(()));
        assert_eq!(
            safety.start_llm_step(),
            Err(LoopSafetyError::MaxLlmStepsExceeded { max: 1 })
        );
        assert_eq!(safety.llm_steps(), 1);
    }
}
