use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::domain::model::tool_call::ToolSpec;

pub const SUBAGENT_TOOL_NAME: &str = "subagent";
pub const DEFAULT_PROFILE_NAME: &str = "default";
pub const MAX_SUBAGENT_TASKS: usize = 3;

#[derive(Debug, Clone, Deserialize)]
pub struct SubagentInput {
    pub tasks: Vec<SubagentTaskInput>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SubagentTaskInput {
    pub profile: String,
    pub request: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SubagentOutput {
    pub results: Vec<SubagentTaskOutput>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SubagentTaskOutput {
    pub index: usize,
    pub task_id: String,
    pub profile: String,
    pub status: SubagentTaskStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SubagentTaskStatus {
    Completed,
    Failed,
    Cancelled,
}

pub struct AgentProfile {
    pub name: &'static str,
    pub instruction: &'static str,
    pub allowed_tools: &'static [&'static str],
}

pub const DEFAULT_PROFILE: AgentProfile = AgentProfile {
    name: DEFAULT_PROFILE_NAME,
    instruction: "You are a focused child agent. Handle the given child task independently, gather evidence, analyze, compare, and summarize the result. Do not modify files or perform write actions.",
    allowed_tools: &[
        "file_list",
        "file_read",
        "file_search",
        "text_search",
        "web_search",
        "web_fetch",
    ],
};

pub fn find_profile(name: &str) -> Option<&'static AgentProfile> {
    match name {
        DEFAULT_PROFILE_NAME => Some(&DEFAULT_PROFILE),
        _ => None,
    }
}

pub fn supported_profile_names() -> &'static [&'static str] {
    &[DEFAULT_PROFILE_NAME]
}

pub fn parse_input(arguments: Value) -> Result<SubagentInput, String> {
    let mut input = serde_json::from_value::<SubagentInput>(arguments)
        .map_err(|err| format!("invalid subagent arguments: {err}"))?;

    for task in &mut input.tasks {
        task.profile = task.profile.trim().to_string();
        task.request = task.request.trim().to_string();
    }

    validate_input(&input)?;

    Ok(input)
}

fn validate_input(input: &SubagentInput) -> Result<(), String> {
    if input.tasks.is_empty() {
        return Err("subagent requires at least one task".to_string());
    }

    if input.tasks.len() > MAX_SUBAGENT_TASKS {
        return Err(format!(
            "subagent accepts at most {MAX_SUBAGENT_TASKS} tasks"
        ));
    }

    for (index, task) in input.tasks.iter().enumerate() {
        if task.profile.is_empty() {
            return Err(format!("tasks[{index}].profile must not be empty"));
        }

        if task.request.is_empty() {
            return Err(format!("tasks[{index}].request must not be empty"));
        }

        if find_profile(&task.profile).is_none() {
            return Err(format!(
                "unsupported profile: {}. Supported profiles: {}",
                task.profile,
                supported_profile_names().join(", ")
            ));
        }
    }

    Ok(())
}

pub fn tool_spec() -> ToolSpec {
    ToolSpec {
        name: SUBAGENT_TOOL_NAME.to_string(),
        description: "Run focused child tasks in parallel and return their final results. Available profiles: default.".to_string(),
        parameters: json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "tasks": {
                    "type": "array",
                    "minItems": 1,
                    "maxItems": MAX_SUBAGENT_TASKS,
                    "items": {
                        "type": "object",
                        "additionalProperties": false,
                        "properties": {
                            "profile": {
                                "type": "string",
                                "minLength": 1,
                                "description": "Registered child-agent profile name. Currently supported: default."
                            },
                            "request": {
                                "type": "string",
                                "minLength": 1,
                                "description": "Concrete request for the child task."
                            }
                        },
                        "required": ["profile", "request"]
                    }
                }
            },
            "required": ["tasks"]
        }),
    }
}
