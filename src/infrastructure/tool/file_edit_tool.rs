use std::path::{Path, PathBuf};

use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::fs;

use crate::domain::error::tool_executor_error::ToolExecutorError;
use crate::domain::model::tool_call::ToolPermissionMode;
use crate::domain::port::tool::Tool;

#[derive(Debug, Clone)]
pub struct FileEditTool {
    workspace_root: PathBuf,
}

impl FileEditTool {
    pub fn new(workspace_root: PathBuf) -> Self {
        Self { workspace_root }
    }

    async fn resolve_path(&self, path: &str) -> Result<PathBuf, ToolExecutorError> {
        let path = path.trim();

        if path.is_empty() {
            return Err(ToolExecutorError::InvalidArguments(
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
            .map_err(|err| ToolExecutorError::ExecutionFailed(err.to_string()))?;

        let resolved = fs::canonicalize(joined)
            .await
            .map_err(|err| ToolExecutorError::ExecutionFailed(err.to_string()))?;

        if !resolved.starts_with(&workspace_root) {
            return Err(ToolExecutorError::ExecutionFailed(format!(
                "path is outside workspace: {path}"
            )));
        }

        Ok(resolved)
    }
}

#[derive(Debug, Deserialize)]
struct FileEditArguments {
    path: String,
    edits: Vec<TextEdit>,
}

#[derive(Debug, Deserialize)]
struct TextEdit {
    old_text: String,
    new_text: String,
}

#[async_trait]
impl Tool for FileEditTool {
    fn name(&self) -> &'static str {
        "file_edit"
    }

    fn description(&self) -> &'static str {
        "Edit a UTF-8 text file inside the workspace by applying exact text replacements in order. If any replacement fails, the file is not changed."
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
                    "description": "File path to edit. Prefer a path relative to the workspace root."
                },
                "edits": {
                    "type": "array",
                    "minItems": 1,
                    "description": "Exact text replacements to apply in order.",
                    "items": {
                        "type": "object",
                        "properties": {
                            "old_text": {
                                "type": "string",
                                "description": "Exact text to replace. It must appear exactly once when this edit is applied."
                            },
                            "new_text": {
                                "type": "string",
                                "description": "Replacement text."
                            }
                        },
                        "required": ["old_text", "new_text"],
                        "additionalProperties": false
                    }
                }
            },
            "required": ["path", "edits"],
            "additionalProperties": false
        })
    }

    async fn execute(&self, arguments: Value) -> Result<Value, ToolExecutorError> {
        let args: FileEditArguments = serde_json::from_value(arguments)
            .map_err(|err| ToolExecutorError::InvalidArguments(err.to_string()))?;

        if args.edits.is_empty() {
            return Err(ToolExecutorError::InvalidArguments(
                "edits must not be empty".to_string(),
            ));
        }

        let path = self.resolve_path(&args.path).await?;

        let metadata = fs::metadata(&path)
            .await
            .map_err(|err| ToolExecutorError::ExecutionFailed(err.to_string()))?;

        if !metadata.is_file() {
            return Err(ToolExecutorError::ExecutionFailed(format!(
                "path is not a file: {}",
                args.path
            )));
        }

        if let Ok(metadata) = fs::symlink_metadata(&path).await
            && metadata.file_type().is_symlink()
        {
            return Err(ToolExecutorError::ExecutionFailed(format!(
                "refusing to edit symlink: {}",
                args.path
            )));
        }

        let bytes = fs::read(&path)
            .await
            .map_err(|err| ToolExecutorError::ExecutionFailed(err.to_string()))?;

        let mut content = String::from_utf8(bytes).map_err(|_| {
            ToolExecutorError::ExecutionFailed("file is not valid UTF-8".to_string())
        })?;

        for (index, edit) in args.edits.iter().enumerate() {
            if edit.old_text.is_empty() {
                return Err(ToolExecutorError::InvalidArguments(format!(
                    "edits[{index}].old_text must not be empty"
                )));
            }

            if edit.old_text == edit.new_text {
                return Err(ToolExecutorError::InvalidArguments(format!(
                    "edits[{index}].old_text and new_text must be different"
                )));
            }

            let count = content.matches(&edit.old_text).count();

            if count == 0 {
                return Err(ToolExecutorError::ExecutionFailed(format!(
                    "edits[{index}].old_text was not found"
                )));
            }

            if count > 1 {
                return Err(ToolExecutorError::ExecutionFailed(format!(
                    "edits[{index}].old_text matched {count} times; provide more surrounding context"
                )));
            }

            content = content.replacen(&edit.old_text, &edit.new_text, 1);
        }

        fs::write(&path, content.as_bytes())
            .await
            .map_err(|err| ToolExecutorError::ExecutionFailed(err.to_string()))?;

        Ok(json!({
            "path": args.path,
            "applied": args.edits.len()
        }))
    }
}
