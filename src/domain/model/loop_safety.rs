use crate::domain::error::loop_safety_error::LoopSafetyError;
use crate::domain::model::tool_call::ToolCall;
use crate::domain::model::tool_call_output::ToolCallOutput;

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

    /// A repeated failed tool call must not continue indefinitely within one agent turn.
    pub fn record_tool_call_output(
        &mut self,
        tool_call: &ToolCall,
        output: &ToolCallOutput,
    ) -> Result<(), LoopSafetyError> {
        if output.is_success() {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::model::tool_call::ToolCall;
    use crate::domain::model::tool_call_output::ToolCallOutput;
    use serde_json::json;

    fn tool_call(command: &str) -> ToolCall {
        ToolCall {
            call_id: "call-1".to_string(),
            name: "shell_exec".to_string(),
            arguments: json!({ "command": command }),
        }
    }

    fn error_output() -> ToolCallOutput {
        ToolCallOutput::error("call-1", json!({ "message": "failed" }))
    }

    fn success_output() -> ToolCallOutput {
        ToolCallOutput::success("call-1", json!({ "message": "ok" }))
    }

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

    #[test]
    fn repeated_failed_tool_call_is_rejected_at_the_limit() {
        let mut safety = LoopSafety::new(20);
        let call = tool_call("missing-command");

        for _ in 0..4 {
            assert_eq!(
                safety.record_tool_call_output(&call, &error_output()),
                Ok(())
            );
        }

        assert_eq!(
            safety.record_tool_call_output(&call, &error_output()),
            Err(LoopSafetyError::RepeatedFailedToolCall {
                tool_name: "shell_exec".to_string(),
                repeats: 5,
            })
        );
    }

    #[test]
    fn successful_tool_call_resets_failed_repeats() {
        let mut safety = LoopSafety::new(20);
        let call = tool_call("missing-command");

        for _ in 0..4 {
            assert_eq!(
                safety.record_tool_call_output(&call, &error_output()),
                Ok(())
            );
        }

        assert_eq!(
            safety.record_tool_call_output(&call, &success_output()),
            Ok(())
        );

        for _ in 0..4 {
            assert_eq!(
                safety.record_tool_call_output(&call, &error_output()),
                Ok(())
            );
        }
    }

    #[test]
    fn different_failed_tool_call_resets_failed_repeats() {
        let mut safety = LoopSafety::new(20);
        let first = tool_call("missing-command");
        let second = tool_call("another-missing-command");

        for _ in 0..4 {
            assert_eq!(
                safety.record_tool_call_output(&first, &error_output()),
                Ok(())
            );
        }

        assert_eq!(
            safety.record_tool_call_output(&second, &error_output()),
            Ok(())
        );

        for _ in 0..3 {
            assert_eq!(
                safety.record_tool_call_output(&second, &error_output()),
                Ok(())
            );
        }
    }
}
