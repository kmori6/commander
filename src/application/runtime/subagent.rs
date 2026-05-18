use crate::domain::model::task::{Task, TaskStatus};
use crate::domain::model::tool_call::ToolSpec;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::path::Path;

pub const TOOL_NAME: &str = "subagent";
const MAX_TASKS: usize = 5;

#[derive(Debug, Clone, Deserialize)]
pub struct Input {
    pub tasks: Vec<TaskInput>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TaskInput {
    pub profile: String,
    pub request: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Output {
    pub results: Vec<TaskOutput>,
}

impl Output {
    pub fn from_tasks(tasks: &[Task]) -> Self {
        let results = tasks
            .iter()
            .enumerate()
            .map(|(index, task)| TaskOutput {
                index,
                task_id: task.id.to_string(),
                profile: task.subagent_profile.clone().unwrap_or_default(),
                status: match task.status {
                    TaskStatus::Completed => Status::Completed,
                    TaskStatus::Cancelled => Status::Cancelled,
                    _ => Status::Failed,
                },
                output: (task.status == TaskStatus::Completed).then(|| task.output.clone()),
                error: if task.status == TaskStatus::Completed {
                    None
                } else {
                    task.error
                        .clone()
                        .or_else(|| Some(task.status.as_str().to_string()))
                },
            })
            .collect();

        Self { results }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct TaskOutput {
    pub index: usize,
    pub task_id: String,
    pub profile: String,
    pub status: Status,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Deserialize)]
struct ProfileFile {
    #[serde(default)]
    description: String,
    instruction: String,
    #[serde(default)]
    allowed_tools: Vec<String>,
}

impl Profile {
    fn from_file(name: &str, file: ProfileFile) -> Option<Self> {
        let instruction = file.instruction.trim().to_string();
        let allowed_tools = file
            .allowed_tools
            .into_iter()
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
            .collect::<Vec<_>>();

        if name.trim().is_empty() || instruction.is_empty() || allowed_tools.is_empty() {
            return None;
        }

        Some(Self {
            name: name.trim().to_string(),
            description: file.description.trim().to_string(),
            instruction,
            allowed_tools,
        })
    }

    pub fn allows_tool(&self, tool_name: &str) -> bool {
        self.allowed_tools.iter().any(|tool| tool == tool_name)
    }

    fn summary(&self) -> String {
        if self.description.is_empty() {
            self.name.clone()
        } else {
            format!("{}: {}", self.name, self.description)
        }
    }
}

#[derive(Debug, Clone)]
pub struct Profile {
    pub name: String,
    pub description: String,
    pub instruction: String,
    pub allowed_tools: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct Registry {
    profiles: Vec<Profile>,
}

impl Registry {
    pub fn load(workspace_root: &Path) -> Self {
        let root = workspace_root.join("subagents");
        let mut profiles = Vec::new();

        if let Ok(entries) = std::fs::read_dir(root) {
            for entry in entries.flatten() {
                let path = entry.path();

                if path.extension().and_then(|v| v.to_str()) != Some("json") {
                    continue;
                }

                let Some(name) = path.file_stem().and_then(|v| v.to_str()) else {
                    continue;
                };

                let Ok(content) = std::fs::read_to_string(&path) else {
                    continue;
                };

                match serde_json::from_str::<ProfileFile>(&content) {
                    Ok(file) => {
                        if let Some(profile) = Profile::from_file(name, file) {
                            profiles.push(profile);
                        }
                    }
                    Err(err) => {
                        log::warn!("invalid subagent profile {}: {err}", path.display());
                    }
                }
            }
        }

        profiles.sort_by(|a, b| a.name.cmp(&b.name));
        Self { profiles }
    }

    pub fn find(&self, name: &str) -> Option<&Profile> {
        self.profiles.iter().find(|profile| profile.name == name)
    }

    fn validate_input(&self, input: &Input) -> Result<(), String> {
        if input.tasks.is_empty() {
            return Err("subagent requires at least one task".to_string());
        }

        if input.tasks.len() > MAX_TASKS {
            return Err(format!("subagent supports at most {MAX_TASKS} tasks"));
        }

        let supported = self
            .profiles
            .iter()
            .map(|profile| profile.name.as_str())
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

    pub fn parse_input(&self, arguments: Value) -> Result<Input, String> {
        let mut input = serde_json::from_value::<Input>(arguments)
            .map_err(|err| format!("invalid subagent arguments: {err}"))?;

        for task in &mut input.tasks {
            task.profile = task.profile.trim().to_string();
            task.request = task.request.trim().to_string();
        }

        self.validate_input(&input)?;
        Ok(input)
    }

    pub fn tool_spec(&self) -> Option<ToolSpec> {
        if self.profiles.is_empty() {
            return None;
        }

        let names = self
            .profiles
            .iter()
            .map(|p| p.name.clone())
            .collect::<Vec<_>>();

        let descriptions = self
            .profiles
            .iter()
            .map(|p| p.summary())
            .collect::<Vec<_>>()
            .join(", ");

        Some(ToolSpec {
            name: TOOL_NAME.to_string(),
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
}
