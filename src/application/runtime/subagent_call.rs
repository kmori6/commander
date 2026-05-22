use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::domain::model::subagent::Subagent;
use crate::domain::model::tool_call::ToolSpec;

const MAX_TASKS: usize = 5;

#[derive(Debug, Clone)]
pub struct SubagentCall {
    subagents: Vec<Subagent>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SubagentCallInput {
    pub tasks: Vec<SubagentRequest>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SubagentRequest {
    pub profile: String,
    pub request: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SubagentResult {
    pub index: usize,
    pub profile: String,
    pub status: SubagentResultStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SubagentResultStatus {
    Completed,
    Failed,
    Cancelled,
}

impl SubagentCall {
    pub const TOOL_NAME: &'static str = "subagent";

    pub fn new(subagents: Vec<Subagent>) -> Self {
        Self { subagents }
    }

    pub fn find(&self, name: &str) -> Option<&Subagent> {
        self.subagents.iter().find(|subagent| subagent.name == name)
    }

    pub fn parse(&self, arguments: Value) -> Result<SubagentCallInput, String> {
        let mut input = serde_json::from_value::<SubagentCallInput>(arguments)
            .map_err(|err| format!("invalid subagent arguments: {err}"))?;

        for task in &mut input.tasks {
            task.profile = task.profile.trim().to_string();
            task.request = task.request.trim().to_string();
        }

        self.validate(&input)?;
        Ok(input)
    }

    pub fn output(results: Vec<SubagentResult>) -> Value {
        json!({ "results": results })
    }

    pub fn tool_spec(&self) -> Option<ToolSpec> {
        if self.subagents.is_empty() {
            return None;
        }

        let names = self
            .subagents
            .iter()
            .map(|subagent| subagent.name.clone())
            .collect::<Vec<_>>();

        let descriptions = self
            .subagents
            .iter()
            .map(Subagent::summary)
            .collect::<Vec<_>>()
            .join(", ");

        Some(ToolSpec {
            name: Self::TOOL_NAME.to_string(),
            description: format!(
                "Run focused child tasks and return their results to the parent agent. Available profiles: {descriptions}."
            ),
            parameters: json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "tasks": {
                        "type": "array",
                        "minItems": 1,
                        "maxItems": MAX_TASKS,
                        "items": {
                            "type": "object",
                            "additionalProperties": false,
                            "properties": {
                                "profile": { "type": "string", "enum": names },
                                "request": { "type": "string", "minLength": 1 }
                            },
                            "required": ["profile", "request"]
                        }
                    }
                },
                "required": ["tasks"]
            }),
        })
    }

    fn validate(&self, input: &SubagentCallInput) -> Result<(), String> {
        if input.tasks.is_empty() {
            return Err("subagent requires at least one task".to_string());
        }

        if input.tasks.len() > MAX_TASKS {
            return Err(format!("subagent supports at most {MAX_TASKS} tasks"));
        }

        let supported = self
            .subagents
            .iter()
            .map(|subagent| subagent.name.as_str())
            .collect::<Vec<_>>()
            .join(", ");

        for (index, task) in input.tasks.iter().enumerate() {
            if task.profile.is_empty() {
                return Err(format!("tasks[{index}].profile must not be empty"));
            }

            if task.request.is_empty() {
                return Err(format!("tasks[{index}].request must not be empty"));
            }

            if self.find(&task.profile).is_none() {
                return Err(format!(
                    "unsupported profile: {}. Supported profiles: {}",
                    task.profile, supported
                ));
            }
        }

        Ok(())
    }
}
