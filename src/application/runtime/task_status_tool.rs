use crate::domain::model::task::{Task, TaskStatus};
use crate::domain::model::tool_call::ToolSpec;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

pub const TASK_STATUS_TOOL_NAME: &str = "task_status";

const DEFAULT_TASK_STATUS_LIMIT: usize = 20;
const MAX_TASK_STATUS_LIMIT: usize = 100;

#[derive(Debug, Clone, Deserialize)]
pub struct TaskStatusInput {
    #[serde(default)]
    pub task_id: Option<String>,
    #[serde(default)]
    pub status: Option<TaskStatus>,
    #[serde(default = "default_task_status_limit")]
    pub limit: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct TaskStatusOutput {
    pub count: usize,
    pub tasks: Vec<TaskStatusTaskOutput>,
}

impl TaskStatusOutput {
    pub fn new(tasks: Vec<TaskStatusTaskOutput>) -> Self {
        Self {
            count: tasks.len(),
            tasks,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct TaskStatusTaskOutput {
    pub task_id: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<TaskStatusResultOutput>,
    pub created_at: String,
    pub updated_at: String,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TaskStatusResultOutput {
    pub error: Option<String>,
}

impl TaskStatusTaskOutput {
    pub fn from_task(task: &Task, include_result: bool) -> Self {
        let result = include_result.then(|| TaskStatusResultOutput {
            error: task.error.clone(),
        });

        Self {
            task_id: task.id.to_string(),
            status: task.status.as_str().to_string(),
            result,
            created_at: task.created_at.to_rfc3339(),
            updated_at: task.updated_at.to_rfc3339(),
            started_at: task.started_at.map(|dt| dt.to_rfc3339()),
            finished_at: task.finished_at.map(|dt| dt.to_rfc3339()),
        }
    }
}

pub fn parse_task_status_input(arguments: Value) -> Result<TaskStatusInput, String> {
    let mut input = serde_json::from_value::<TaskStatusInput>(arguments)
        .map_err(|err| format!("invalid task_status arguments: {err}"))?;

    input.task_id = input
        .task_id
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty());

    if input.limit == 0 {
        input.limit = DEFAULT_TASK_STATUS_LIMIT;
    }
    input.limit = input.limit.min(MAX_TASK_STATUS_LIMIT);

    Ok(input)
}

pub fn task_status_tool_spec() -> ToolSpec {
    ToolSpec {
        name: TASK_STATUS_TOOL_NAME.to_string(),
        description: "Get task status. With task_id, returns one task with error details. With no task_id, lists recent tasks.".to_string(),
        parameters: json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "task_id": { "type": "string" },
                "status": {
                    "type": "string",
                    "enum": ["queued", "running", "awaiting_approval", "completed", "failed", "cancelled"]
                },
                "limit": {
                    "type": "integer",
                    "minimum": 1,
                    "maximum": MAX_TASK_STATUS_LIMIT
                }
            },
            "required": []
        }),
    }
}

fn default_task_status_limit() -> usize {
    DEFAULT_TASK_STATUS_LIMIT
}
