use std::path::{Path, PathBuf};

use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::fs;

use crate::domain::error::tool_error::ToolError;
use crate::domain::model::tool_call::ToolPermissionMode;
use crate::domain::port::tool::Tool;

const MAX_ENTRIES: usize = 200;

#[derive(Debug, Clone)]
pub struct FileListTool {
    workspace_root: PathBuf,
}

impl FileListTool {
    pub fn new(workspace_root: PathBuf) -> Self {
        Self { workspace_root }
    }

    async fn resolve_path(&self, path: &str) -> Result<(PathBuf, PathBuf), ToolError> {
        let path = path.trim();
        let requested = if path.is_empty() {
            Path::new(".")
        } else {
            Path::new(path)
        };

        let workspace_root = fs::canonicalize(&self.workspace_root)
            .await
            .map_err(|err| ToolError::ExecutionFailed(err.to_string()))?;

        let joined = if requested.is_absolute() {
            requested.to_path_buf()
        } else {
            workspace_root.join(requested)
        };

        let resolved = fs::canonicalize(joined)
            .await
            .map_err(|err| ToolError::ExecutionFailed(err.to_string()))?;

        if !resolved.starts_with(&workspace_root) {
            return Err(ToolError::ExecutionFailed(format!(
                "path is outside workspace: {path}"
            )));
        }

        Ok((workspace_root, resolved))
    }
}

#[derive(Debug, Deserialize)]
struct FileListArguments {
    path: Option<String>,
}

#[derive(Debug)]
struct FileListEntry {
    path: String,
    name: String,
    kind: &'static str,
}

#[async_trait]
impl Tool for FileListTool {
    fn name(&self) -> &'static str {
        "file_list"
    }

    fn description(&self) -> &'static str {
        "List files and directories directly inside a workspace directory."
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
                    "description": "Directory path to list. Defaults to the workspace root."
                }
            },
            "additionalProperties": false
        })
    }

    async fn execute(&self, arguments: Value) -> Result<Value, ToolError> {
        let args: FileListArguments = serde_json::from_value(arguments)
            .map_err(|err| ToolError::InvalidArguments(err.to_string()))?;

        let requested_path = args.path.unwrap_or_else(|| ".".to_string());
        let (workspace_root, path) = self.resolve_path(&requested_path).await?;

        let metadata = fs::metadata(&path)
            .await
            .map_err(|err| ToolError::ExecutionFailed(err.to_string()))?;

        if !metadata.is_dir() {
            return Err(ToolError::ExecutionFailed(format!(
                "path is not a directory: {requested_path}"
            )));
        }

        let mut read_dir = fs::read_dir(&path)
            .await
            .map_err(|err| ToolError::ExecutionFailed(err.to_string()))?;

        let mut entries = Vec::new();

        while let Some(entry) = read_dir
            .next_entry()
            .await
            .map_err(|err| ToolError::ExecutionFailed(err.to_string()))?
        {
            let entry_path = entry.path();
            let Ok(resolved_entry_path) = fs::canonicalize(&entry_path).await else {
                continue;
            };

            if !resolved_entry_path.starts_with(&workspace_root) {
                continue;
            }

            let metadata = entry
                .metadata()
                .await
                .map_err(|err| ToolError::ExecutionFailed(err.to_string()))?;

            let kind = if metadata.is_dir() {
                "directory"
            } else if metadata.is_file() {
                "file"
            } else {
                "other"
            };

            let name = entry.file_name().to_string_lossy().to_string();

            entries.push(FileListEntry {
                path: relative_path(&workspace_root, &entry_path)?,
                name,
                kind,
            });
        }

        entries.sort_by(|a, b| {
            let a_rank = kind_rank(a.kind);
            let b_rank = kind_rank(b.kind);

            a_rank.cmp(&b_rank).then_with(|| a.name.cmp(&b.name))
        });

        let truncated = entries.len() > MAX_ENTRIES;
        entries.truncate(MAX_ENTRIES);

        Ok(json!({
            "path": relative_path(&workspace_root, &path)?,
            "entries": entries
                .into_iter()
                .map(|entry| {
                    json!({
                        "path": entry.path,
                        "name": entry.name,
                        "kind": entry.kind
                    })
                })
                .collect::<Vec<_>>(),
            "truncated": truncated
        }))
    }
}

fn relative_path(workspace_root: &Path, path: &Path) -> Result<String, ToolError> {
    let relative = path.strip_prefix(workspace_root).map_err(|err| {
        ToolError::ExecutionFailed(format!("failed to build relative path: {err}"))
    })?;

    if relative.as_os_str().is_empty() {
        Ok(".".to_string())
    } else {
        Ok(relative.to_string_lossy().replace('\\', "/"))
    }
}

fn kind_rank(kind: &str) -> u8 {
    match kind {
        "directory" => 0,
        "file" => 1,
        _ => 2,
    }
}
