use crate::domain::model::task::Task;
use crate::domain::model::tool_call::ToolSpec;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::path::Path;

pub const SUBAGENT_TOOL_NAME: &str = "subagent";
pub const SUBAGENT_STATUS_TOOL_NAME: &str = "subagent_status";

const DEFAULT_SUBAGENT_STATUS_LIMIT: usize = 20;
const MAX_SUBAGENT_STATUS_LIMIT: usize = 100;
const SUBAGENT_STATUS_OUTPUT_MAX_CHARS: usize = 4000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum SubagentMode {
    #[default]
    Wait,
    Spawn,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SubagentInput {
    #[serde(default)]
    pub mode: SubagentMode,
    pub tasks: Vec<SubagentTaskInput>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SubagentTaskInput {
    pub profile: String,
    pub request: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SubagentOutput {
    pub mode: SubagentMode,
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
    Spawned,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SubagentStatusInput {
    #[serde(default)]
    pub task_id: Option<String>,
    #[serde(default = "default_subagent_status_limit")]
    pub limit: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct SubagentStatusOutput {
    pub count: usize,
    pub tasks: Vec<SubagentStatusTaskOutput>,
}

impl SubagentStatusOutput {
    pub fn new(tasks: Vec<SubagentStatusTaskOutput>) -> Self {
        Self {
            count: tasks.len(),
            tasks,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct SubagentStatusTaskOutput {
    pub task_id: String,
    pub parent_task_id: Option<String>,
    pub status: String,
    pub request: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<String>,
    pub output_truncated: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finished_at: Option<String>,
}

impl SubagentStatusTaskOutput {
    pub fn from_task(task: &Task) -> Self {
        let (output, output_truncated) = if task.output.is_empty() {
            (None, false)
        } else {
            let (output, truncated) =
                truncate_chars(&task.output, SUBAGENT_STATUS_OUTPUT_MAX_CHARS);
            (Some(output), truncated)
        };

        Self {
            task_id: task.id.to_string(),
            parent_task_id: task.parent_task_id.map(|id| id.to_string()),
            status: task.status.as_str().to_string(),
            request: task.request.clone(),
            output,
            output_truncated,
            error: task.error.clone(),
            created_at: task.created_at.to_rfc3339(),
            updated_at: task.updated_at.to_rfc3339(),
            started_at: task.started_at.map(|dt| dt.to_rfc3339()),
            finished_at: task.finished_at.map(|dt| dt.to_rfc3339()),
        }
    }
}

pub fn parse_subagent_status_input(arguments: Value) -> Result<SubagentStatusInput, String> {
    let mut input = serde_json::from_value::<SubagentStatusInput>(arguments)
        .map_err(|err| format!("invalid subagent_status arguments: {err}"))?;

    input.task_id = input
        .task_id
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());

    if input.limit == 0 {
        input.limit = DEFAULT_SUBAGENT_STATUS_LIMIT;
    }

    input.limit = input.limit.min(MAX_SUBAGENT_STATUS_LIMIT);

    Ok(input)
}

pub fn subagent_status_tool_spec() -> ToolSpec {
    ToolSpec {
        name: SUBAGENT_STATUS_TOOL_NAME.to_string(),
        description: "Get status and results for child tasks spawned by the current task. Without task_id, lists child tasks for the current task. With task_id, returns that child task if it belongs to the current task.".to_string(),
        parameters: json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "task_id": {
                    "type": "string",
                    "description": "Optional child task ID to inspect."
                },
                "limit": {
                    "type": "integer",
                    "minimum": 1,
                    "maximum": MAX_SUBAGENT_STATUS_LIMIT,
                    "description": "Maximum number of child tasks to return when task_id is omitted. Defaults to 20."
                }
            },
            "required": []
        }),
    }
}

fn default_subagent_status_limit() -> usize {
    DEFAULT_SUBAGENT_STATUS_LIMIT
}

fn truncate_chars(text: &str, max_chars: usize) -> (String, bool) {
    if text.chars().count() <= max_chars {
        return (text.to_string(), false);
    }

    let truncated = text.chars().take(max_chars).collect::<String>();
    (format!("{truncated}\n... [truncated]"), true)
}

#[derive(Debug, Deserialize)]
struct SubagentProfileFile {
    #[serde(default)]
    description: String,
    instruction: String,
    #[serde(default)]
    allowed_tools: Vec<String>,
}

impl SubagentProfile {
    fn from_file(name: &str, file: SubagentProfileFile) -> Option<Self> {
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
pub struct SubagentProfile {
    pub name: String,
    pub description: String,
    pub instruction: String,
    pub allowed_tools: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct Subagents {
    profiles: Vec<SubagentProfile>,
}

impl Subagents {
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

                match serde_json::from_str::<SubagentProfileFile>(&content) {
                    Ok(file) => {
                        if let Some(profile) = SubagentProfile::from_file(name, file) {
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

    pub fn find(&self, name: &str) -> Option<&SubagentProfile> {
        self.profiles.iter().find(|profile| profile.name == name)
    }

    fn validate_input(&self, input: &SubagentInput) -> Result<(), String> {
        if input.tasks.is_empty() {
            return Err("subagent requires at least one task".to_string());
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

    pub fn parse_input(&self, arguments: Value) -> Result<SubagentInput, String> {
        let mut input = serde_json::from_value::<SubagentInput>(arguments)
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
            name: SUBAGENT_TOOL_NAME.to_string(),
            description: format!(
                "Run focused child tasks. mode=wait runs tasks in parallel and returns final results; mode=spawn starts tasks and returns task IDs immediately. Available profiles: {descriptions}."
            ),
            parameters: json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "mode": {
                        "type": "string",
                        "enum": ["wait", "spawn"],
                        "description": "wait returns final results. spawn starts child tasks and returns task IDs immediately. Defaults to wait."
                    },
                    "tasks": {
                        "type": "array",
                        "minItems": 1,
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
