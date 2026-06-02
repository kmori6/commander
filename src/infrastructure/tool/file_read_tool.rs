use std::path::{Path, PathBuf};

use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::fs;

use crate::domain::error::tool_error::ToolError;
use crate::domain::model::tool_call::ToolPermissionMode;
use crate::domain::port::tool::Tool;

const MAX_LINES: usize = 10_000;
const MAX_BYTES: usize = 100_000;

#[derive(Debug, Clone)]
pub struct FileReadTool {
    workspace_root: PathBuf,
}

impl FileReadTool {
    pub fn new(workspace_root: PathBuf) -> Self {
        Self { workspace_root }
    }

    async fn resolve_path(&self, path: &str) -> Result<PathBuf, ToolError> {
        let path = path.trim();

        if path.is_empty() {
            return Err(ToolError::InvalidArguments(
                "path must not be empty".to_string(),
            ));
        }

        let requested = Path::new(path);
        let joined = if requested.is_absolute() {
            requested.to_path_buf()
        } else {
            self.workspace_root.join(requested)
        };

        let workspace_root = fs::canonicalize(&self.workspace_root)
            .await
            .map_err(|err| ToolError::ExecutionFailed(err.to_string()))?;

        let resolved = fs::canonicalize(joined)
            .await
            .map_err(|err| ToolError::ExecutionFailed(err.to_string()))?;

        if !resolved.starts_with(&workspace_root) {
            return Err(ToolError::ExecutionFailed(format!(
                "path is outside workspace: {path}"
            )));
        }

        Ok(resolved)
    }
}

#[derive(Debug, Deserialize)]
struct FileReadArguments {
    path: String,
    start_line: Option<usize>,
    end_line: Option<usize>,
}

#[async_trait]
impl Tool for FileReadTool {
    fn name(&self) -> &str {
        "file_read"
    }

    fn description(&self) -> &str {
        "Read selected lines from a UTF-8 text file inside the workspace."
    }

    fn default_permission(&self) -> ToolPermissionMode {
        ToolPermissionMode::Allow
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "File path to read. Prefer a path relative to the workspace root."
                },
                "start_line": {
                    "type": "integer",
                    "minimum": 1,
                    "description": "First line to include, using 1-based line numbers. Defaults to 1."
                },
                "end_line": {
                    "type": "integer",
                    "minimum": 1,
                    "description": "Last line to include, using 1-based line numbers. Omit to read up to the internal limit."
                }
            },
            "required": ["path"],
            "additionalProperties": false
        })
    }

    async fn execute(&self, arguments: Value) -> Result<Value, ToolError> {
        let args: FileReadArguments = serde_json::from_value(arguments)
            .map_err(|err| ToolError::InvalidArguments(err.to_string()))?;

        let path = self.resolve_path(&args.path).await?;

        let metadata = fs::metadata(&path)
            .await
            .map_err(|err| ToolError::ExecutionFailed(err.to_string()))?;

        if !metadata.is_file() {
            return Err(ToolError::ExecutionFailed(format!(
                "path is not a file: {}",
                args.path
            )));
        }

        let bytes = fs::read(&path)
            .await
            .map_err(|err| ToolError::ExecutionFailed(err.to_string()))?;

        let text = String::from_utf8(bytes)
            .map_err(|_| ToolError::ExecutionFailed("file is not valid UTF-8".to_string()))?;

        let start_line = args.start_line.unwrap_or(1).max(1);
        let requested_end_line = args.end_line.unwrap_or(start_line + MAX_LINES - 1);

        if requested_end_line < start_line {
            return Err(ToolError::InvalidArguments(
                "end_line must be greater than or equal to start_line".to_string(),
            ));
        }

        let limited_end_line = requested_end_line.min(start_line + MAX_LINES - 1);

        let mut selected = Vec::new();

        for line in text
            .lines()
            .skip(start_line - 1)
            .take(limited_end_line - start_line + 1)
        {
            selected.push(line);
        }

        let mut content = selected.join("\n");
        let mut truncated = requested_end_line > limited_end_line;

        if content.len() > MAX_BYTES {
            content.truncate(MAX_BYTES);

            while !content.is_char_boundary(content.len()) {
                content.pop();
            }

            truncated = true;
        }

        let actual_end_line = if selected.is_empty() {
            start_line.saturating_sub(1)
        } else {
            start_line + selected.len() - 1
        };

        Ok(json!({
            "path": args.path,
            "content": content,
            "start_line": start_line,
            "end_line": actual_end_line,
            "truncated": truncated
        }))
    }
}
