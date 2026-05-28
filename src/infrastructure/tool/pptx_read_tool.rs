use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::fs;
use tokio::process::Command;
use tokio::time::timeout;

use crate::domain::error::tool_error::ToolError;
use crate::domain::model::tool_call::ToolPermissionMode;
use crate::domain::port::tool::Tool;

const MAX_SOURCE_BYTES: u64 = 1_000_000_000;
const MAX_CONTENT_BYTES: usize = 100_000_000;
const TIMEOUT_SECONDS: u64 = 60;

#[derive(Debug, Clone)]
pub struct PptxReadTool {
    workspace_root: PathBuf,
}

impl PptxReadTool {
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
struct PptxReadArguments {
    path: String,
}

#[async_trait]
impl Tool for PptxReadTool {
    fn name(&self) -> &'static str {
        "pptx_read"
    }

    fn description(&self) -> &'static str {
        "Extract readable text from a PPTX presentation as Markdown."
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
                    "description": "PPTX file path to read. Prefer a path relative to the workspace root."
                }
            },
            "required": ["path"],
            "additionalProperties": false
        })
    }

    async fn execute(&self, arguments: Value) -> Result<Value, ToolError> {
        let args: PptxReadArguments = serde_json::from_value(arguments)
            .map_err(|err| ToolError::InvalidArguments(err.to_string()))?;

        let path = self.resolve_path(&args.path).await?;

        if path.extension().and_then(|ext| ext.to_str()) != Some("pptx") {
            return Err(ToolError::InvalidArguments(
                "path must point to a .pptx file".to_string(),
            ));
        }

        let metadata = fs::metadata(&path)
            .await
            .map_err(|err| ToolError::ExecutionFailed(err.to_string()))?;

        if !metadata.is_file() {
            return Err(ToolError::ExecutionFailed(format!(
                "path is not a file: {}",
                args.path
            )));
        }

        if metadata.len() == 0 {
            return Err(ToolError::ExecutionFailed("pptx file is empty".to_string()));
        }

        if metadata.len() > MAX_SOURCE_BYTES {
            return Err(ToolError::ExecutionFailed(format!(
                "pptx file is too large; limit is {MAX_SOURCE_BYTES} bytes"
            )));
        }

        let output = timeout(
            Duration::from_secs(TIMEOUT_SECONDS),
            Command::new("markitdown")
                .arg(&path)
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .kill_on_drop(true)
                .output(),
        )
        .await
        .map_err(|_| {
            ToolError::ExecutionFailed(format!(
                "markitdown timed out after {TIMEOUT_SECONDS} seconds"
            ))
        })?
        .map_err(|err| ToolError::ExecutionFailed(err.to_string()))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            return Err(ToolError::ExecutionFailed(format!(
                "markitdown failed: {stderr}"
            )));
        }

        let raw_content = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if raw_content.is_empty() {
            return Err(ToolError::ExecutionFailed(
                "pptx contained no extractable text".to_string(),
            ));
        }

        let (content, truncated) = truncate_text(&raw_content, MAX_CONTENT_BYTES);

        Ok(json!({
            "path": args.path,
            "content": content,
            "truncated": truncated
        }))
    }
}

fn truncate_text(text: &str, max_bytes: usize) -> (String, bool) {
    if text.len() <= max_bytes {
        return (text.to_string(), false);
    }

    let mut end = max_bytes;
    while !text.is_char_boundary(end) {
        end -= 1;
    }

    (text[..end].to_string(), true)
}
