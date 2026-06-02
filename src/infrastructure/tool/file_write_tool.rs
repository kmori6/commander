use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{Value, json};
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use tokio::fs;

use crate::domain::error::tool_error::ToolError;
use crate::domain::model::tool_call::ToolPermissionMode;
use crate::domain::port::tool::Tool;

#[derive(Debug, Clone)]
pub struct FileWriteTool {
    workspace_root: PathBuf,
}

impl FileWriteTool {
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

        if requested
            .components()
            .any(|component| matches!(component, std::path::Component::ParentDir))
        {
            return Err(ToolError::InvalidArguments(
                "path must not contain '..'".to_string(),
            ));
        }

        let workspace_root = fs::canonicalize(&self.workspace_root)
            .await
            .map_err(|err| ToolError::ExecutionFailed(err.to_string()))?;

        let resolved = if requested.is_absolute() {
            requested.to_path_buf()
        } else {
            workspace_root.join(requested)
        };

        let parent = resolved.parent().ok_or_else(|| {
            ToolError::InvalidArguments("path must have a parent directory".to_string())
        })?;

        let existing_ancestor = deepest_existing_ancestor(parent).await?;
        let existing_ancestor = fs::canonicalize(existing_ancestor)
            .await
            .map_err(|err| ToolError::ExecutionFailed(err.to_string()))?;

        if !existing_ancestor.starts_with(&workspace_root) {
            return Err(ToolError::ExecutionFailed(format!(
                "path is outside workspace: {path}"
            )));
        }

        if let Ok(metadata) = fs::symlink_metadata(&resolved).await {
            if metadata.file_type().is_symlink() {
                return Err(ToolError::ExecutionFailed(format!(
                    "refusing to write through symlink: {path}"
                )));
            }

            if metadata.is_dir() {
                return Err(ToolError::ExecutionFailed(format!(
                    "path is a directory: {path}"
                )));
            }
        }

        Ok(resolved)
    }
}

#[derive(Debug, Deserialize)]
struct FileWriteArguments {
    path: String,
    content: String,
}

#[async_trait]
impl Tool for FileWriteTool {
    fn name(&self) -> &str {
        "file_write"
    }

    fn description(&self) -> &str {
        "Write UTF-8 text to a file inside the workspace. Creates parent directories as needed and overwrites existing files."
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
                    "description": "File path to write. Prefer a path relative to the workspace root."
                },
                "content": {
                    "type": "string",
                    "description": "UTF-8 text content to write to the file."
                }
            },
            "required": ["path", "content"],
            "additionalProperties": false
        })
    }

    async fn execute(&self, arguments: Value) -> Result<Value, ToolError> {
        let args: FileWriteArguments = serde_json::from_value(arguments)
            .map_err(|err| ToolError::InvalidArguments(err.to_string()))?;

        let path = self.resolve_path(&args.path).await?;

        let parent = path.parent().ok_or_else(|| {
            ToolError::InvalidArguments("path must have a parent directory".to_string())
        })?;

        fs::create_dir_all(parent)
            .await
            .map_err(|err| ToolError::ExecutionFailed(err.to_string()))?;

        fs::write(&path, args.content.as_bytes())
            .await
            .map_err(|err| ToolError::ExecutionFailed(err.to_string()))?;

        Ok(json!({
            "path": args.path,
            "bytes_written": args.content.len()
        }))
    }
}

async fn deepest_existing_ancestor(path: &Path) -> Result<PathBuf, ToolError> {
    let mut current = path.to_path_buf();

    loop {
        match fs::try_exists(&current).await {
            Ok(true) => return Ok(current),
            Ok(false) => {
                current = current.parent().map(Path::to_path_buf).ok_or_else(|| {
                    ToolError::ExecutionFailed(
                        "failed to find existing parent directory".to_string(),
                    )
                })?;
            }
            Err(err) if err.kind() == ErrorKind::NotFound => {
                current = current.parent().map(Path::to_path_buf).ok_or_else(|| {
                    ToolError::ExecutionFailed(
                        "failed to find existing parent directory".to_string(),
                    )
                })?;
            }
            Err(err) => return Err(ToolError::ExecutionFailed(err.to_string())),
        }
    }
}
