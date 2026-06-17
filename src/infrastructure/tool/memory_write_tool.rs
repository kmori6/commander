use async_trait::async_trait;
use chrono::{Local, NaiveDate};
use serde::Deserialize;
use serde_json::{Value, json};
use std::path::PathBuf;
use tokio::io::AsyncWriteExt;

use crate::domain::error::tool_error::ToolError;
use crate::domain::port::tool::Tool;

#[derive(Clone)]
pub struct MemoryWriteTool {
    workspace_root: PathBuf,
}

impl MemoryWriteTool {
    pub fn new(workspace_root: impl Into<PathBuf>) -> Result<Self, ToolError> {
        let workspace_root = std::fs::canonicalize(workspace_root.into())
            .map_err(|err| ToolError::ExecutionFailed(err.to_string()))?;

        Ok(Self { workspace_root })
    }

    fn memory_root(&self) -> PathBuf {
        self.workspace_root.join("memory")
    }

    fn resolve_path(
        &self,
        target: MemoryTarget,
        journal_date: Option<&str>,
    ) -> Result<PathBuf, ToolError> {
        match target {
            MemoryTarget::Memory => Ok(self.memory_root().join("MEMORY.md")),
            MemoryTarget::Journal => {
                let date = match journal_date {
                    Some(value) if !value.trim().is_empty() => {
                        let value = value.trim();
                        NaiveDate::parse_from_str(value, "%Y-%m-%d").map_err(|_| {
                            ToolError::InvalidArguments(
                                "journal_date must use YYYY-MM-DD format".to_string(),
                            )
                        })?;
                        value.to_string()
                    }
                    _ => Local::now().format("%Y-%m-%d").to_string(),
                };

                Ok(self
                    .memory_root()
                    .join("journals")
                    .join(format!("{date}.md")))
            }
        }
    }
}

#[derive(Debug, Deserialize)]
struct MemoryWriteArguments {
    target: MemoryTarget,
    content: String,
    journal_date: Option<String>,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum MemoryTarget {
    Memory,
    Journal,
}

#[async_trait]
impl Tool for MemoryWriteTool {
    fn name(&self) -> &str {
        "memory_write"
    }

    fn description(&self) -> &str {
        "Append a concise note to long-term memory or the daily journal."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "target": {
                    "type": "string",
                    "enum": ["memory", "journal"],
                    "description": "memory is for durable facts and preferences; journal is for work notes and searchable daily context."
                },
                "content": {
                    "type": "string",
                    "description": "Markdown text to append. Write the final note directly."
                },
                "journal_date": {
                    "type": "string",
                    "description": "Journal date in YYYY-MM-DD format. Only used for target=journal. Defaults to today."
                }
            },
            "required": ["target", "content"],
            "additionalProperties": false
        })
    }

    async fn execute(&self, arguments: Value) -> Result<Value, ToolError> {
        let args: MemoryWriteArguments = serde_json::from_value(arguments)
            .map_err(|err| ToolError::InvalidArguments(err.to_string()))?;

        let content = args.content.trim();
        if content.is_empty() {
            return Err(ToolError::InvalidArguments(
                "content must not be empty".to_string(),
            ));
        }

        let path = self.resolve_path(args.target, args.journal_date.as_deref())?;
        let parent = path.parent().ok_or_else(|| {
            ToolError::ExecutionFailed("memory path must include a parent directory".to_string())
        })?;

        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|err| ToolError::ExecutionFailed(err.to_string()))?;

        let existing = match tokio::fs::read_to_string(&path).await {
            Ok(value) => value,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => String::new(),
            Err(err) => return Err(ToolError::ExecutionFailed(err.to_string())),
        };

        let separator = if existing.trim().is_empty() || existing.ends_with("\n\n") {
            ""
        } else if existing.ends_with('\n') {
            "\n"
        } else {
            "\n\n"
        };

        let entry = format!("{separator}{content}\n");

        let mut file = tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .await
            .map_err(|err| ToolError::ExecutionFailed(err.to_string()))?;

        file.write_all(entry.as_bytes())
            .await
            .map_err(|err| ToolError::ExecutionFailed(err.to_string()))?;

        let relative_path = path
            .strip_prefix(&self.workspace_root)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");

        Ok(json!({
            "path": relative_path
        }))
    }
}
