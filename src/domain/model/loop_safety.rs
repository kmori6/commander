use crate::domain::error::loop_safety_error::LoopSafetyError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoopSafety {
    max_llm_steps: usize,
    llm_steps: usize,
}

impl LoopSafety {
    pub fn new(max_llm_steps: usize) -> Self {
        Self {
            max_llm_steps,
            llm_steps: 0,
        }
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

    pub fn llm_steps(&self) -> usize {
        self.llm_steps
    }

    pub fn max_llm_steps(&self) -> usize {
        self.max_llm_steps
    }
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
