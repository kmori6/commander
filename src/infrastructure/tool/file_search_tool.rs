use std::path::{Path, PathBuf};

use async_trait::async_trait;
use glob::{MatchOptions, glob_with};
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::fs;

use crate::domain::error::tool_executor_error::ToolExecutorError;
use crate::domain::model::tool_call::ToolPermissionMode;
use crate::domain::port::tool::Tool;

const MAX_MATCHES: usize = 200;

#[derive(Debug, Clone)]
pub struct FileSearchTool {
    workspace_root: PathBuf,
}

impl FileSearchTool {
    pub fn new(workspace_root: PathBuf) -> Self {
        Self { workspace_root }
    }
}

#[derive(Debug, Deserialize)]
struct FileSearchArguments {
    pattern: String,
}

#[derive(Debug)]
struct FileSearchMatch {
    path: String,
    name: String,
}

#[async_trait]
impl Tool for FileSearchTool {
    fn name(&self) -> &'static str {
        "file_search"
    }

    fn description(&self) -> &'static str {
        "Find workspace files whose paths match a glob pattern."
    }

    fn default_permission(&self) -> ToolPermissionMode {
        ToolPermissionMode::Allow
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "Glob pattern relative to the workspace root, such as src/**/*.rs or data/*.md."
                }
            },
            "required": ["pattern"],
            "additionalProperties": false
        })
    }

    async fn execute(&self, arguments: Value) -> Result<Value, ToolExecutorError> {
        let args: FileSearchArguments = serde_json::from_value(arguments)
            .map_err(|err| ToolExecutorError::InvalidArguments(err.to_string()))?;

        let pattern = args.pattern.trim();

        if pattern.is_empty() {
            return Err(ToolExecutorError::InvalidArguments(
                "pattern must not be empty".to_string(),
            ));
        }

        let pattern_path = Path::new(pattern);

        if pattern_path.is_absolute() {
            return Err(ToolExecutorError::InvalidArguments(
                "pattern must be relative to the workspace root".to_string(),
            ));
        }

        if pattern_path
            .components()
            .any(|component| matches!(component, std::path::Component::ParentDir))
        {
            return Err(ToolExecutorError::InvalidArguments(
                "pattern must not contain '..'".to_string(),
            ));
        }

        let workspace_root = fs::canonicalize(&self.workspace_root)
            .await
            .map_err(|err| ToolExecutorError::ExecutionFailed(err.to_string()))?;

        let full_pattern = workspace_root.join(pattern).to_string_lossy().to_string();

        let options = MatchOptions {
            case_sensitive: true,
            require_literal_separator: false,
            require_literal_leading_dot: false,
        };

        let entries = glob_with(&full_pattern, options)
            .map_err(|err| ToolExecutorError::InvalidArguments(err.to_string()))?;

        let mut matches = Vec::new();

        for entry in entries {
            let Ok(path) = entry else {
                continue;
            };

            let Ok(resolved_path) = fs::canonicalize(&path).await else {
                continue;
            };

            if !resolved_path.starts_with(&workspace_root) {
                continue;
            }

            let Ok(metadata) = fs::metadata(&resolved_path).await else {
                continue;
            };

            if !metadata.is_file() {
                continue;
            }

            let relative_path = relative_path(&workspace_root, &resolved_path)?;
            let name = resolved_path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("")
                .to_string();

            matches.push(FileSearchMatch {
                path: relative_path,
                name,
            });
        }

        matches.sort_by(|a, b| a.path.cmp(&b.path));
        matches.dedup_by(|a, b| a.path == b.path);

        let truncated = matches.len() > MAX_MATCHES;
        matches.truncate(MAX_MATCHES);

        Ok(json!({
            "pattern": pattern,
            "matches": matches
                .into_iter()
                .map(|item| {
                    json!({
                        "path": item.path,
                        "name": item.name
                    })
                })
                .collect::<Vec<_>>(),
            "truncated": truncated
        }))
    }
}

fn relative_path(workspace_root: &Path, path: &Path) -> Result<String, ToolExecutorError> {
    let relative = path.strip_prefix(workspace_root).map_err(|err| {
        ToolExecutorError::ExecutionFailed(format!("failed to build relative path: {err}"))
    })?;

    if relative.as_os_str().is_empty() {
        Ok(".".to_string())
    } else {
        Ok(relative.to_string_lossy().replace('\\', "/"))
    }
}
