use serde::Deserialize;
use serde_json::{Value, json};

use crate::domain::model::subagent::Subagent;
use crate::domain::model::tool_call::ToolSpec;

const MAX_TASKS: usize = 5;

#[derive(Debug, Deserialize)]
struct SubagentToolInput {
    tasks: Vec<SubagentToolTask>,
}

#[derive(Debug, Deserialize)]
struct SubagentToolTask {
    profile: String,
    request: String,
}

#[derive(Debug, Clone)]
pub struct SubagentTool;

impl SubagentTool {
    pub fn name() -> String {
        "subagent".to_string()
    }

    pub fn spec(profiles: &[Subagent]) -> Option<ToolSpec> {
        if profiles.is_empty() {
            return None;
        }

        let names = profiles
            .iter()
            .map(|profile| profile.name.clone())
            .collect::<Vec<_>>();

        let descriptions = profiles
            .iter()
            .map(profile_summary)
            .collect::<Vec<_>>()
            .join(", ");

        Some(ToolSpec {
            name: Self::name(),
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

    pub fn parse_tasks(
        profiles: &[Subagent],
        arguments: Value,
    ) -> Result<Vec<(usize, Subagent, String)>, String> {
        let mut input = serde_json::from_value::<SubagentToolInput>(arguments)
            .map_err(|err| format!("invalid subagent arguments: {err}"))?;

        if input.tasks.is_empty() {
            return Err("subagent requires at least one task".to_string());
        }

        if input.tasks.len() > MAX_TASKS {
            return Err(format!("subagent supports at most {MAX_TASKS} tasks"));
        }

        let supported = profiles
            .iter()
            .map(|profile| profile.name.as_str())
            .collect::<Vec<_>>()
            .join(", ");

        let mut tasks = Vec::new();

        for (index, task) in input.tasks.iter_mut().enumerate() {
            task.profile = task.profile.trim().to_string();
            task.request = task.request.trim().to_string();

            if task.profile.is_empty() {
                return Err(format!("tasks[{index}].profile must not be empty"));
            }

            if task.request.is_empty() {
                return Err(format!("tasks[{index}].request must not be empty"));
            }

            let profile = profiles
                .iter()
                .find(|profile| profile.name == task.profile)
                .cloned()
                .ok_or_else(|| {
                    format!(
                        "unsupported profile: {}. Supported profiles: {}",
                        task.profile, supported
                    )
                })?;

            tasks.push((index, profile, task.request.clone()));
        }

        Ok(tasks)
    }
}

fn profile_summary(profile: &Subagent) -> String {
    if profile.description.is_empty() {
        profile.name.clone()
    } else {
        format!("{}: {}", profile.name, profile.description)
    }
}
