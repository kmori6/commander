use crate::domain::error::tool_error::ToolError;
use crate::domain::model::tool::ToolExecutionResult;
use crate::domain::port::tool::Tool;
use async_trait::async_trait;
use serde_json::{Value, json};
use std::path::PathBuf;

const DEFAULT_MAX_CHARS: usize = 50_000;
const MAX_OUTPUT_CHARS: usize = 200_000;

pub struct PdfFileReadTool {
    workspace_root: PathBuf,
}

impl PdfFileReadTool {
    pub fn new(workspace_root: impl Into<PathBuf>) -> Result<Self, ToolError> {
        let workspace_root = std::fs::canonicalize(workspace_root.into()).map_err(|err| {
            ToolError::Unavailable(format!("failed to resolve workspace root: {err}"))
        })?;

        if !workspace_root.is_dir() {
            return Err(ToolError::Unavailable(
                "workspace root must be a directory".into(),
            ));
        }

        Ok(Self { workspace_root })
    }
}

#[async_trait]
impl Tool for PdfFileReadTool {
    fn name(&self) -> &str {
        "pdf_read"
    }

    fn description(&self) -> &str {
        "Read a PDF (.pdf) file from the workspace and return its extracted Markdown text."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path to the .pdf file. Relative paths are resolved from the workspace root."
                },
                "max_chars": {
                    "type": "integer",
                    "minimum": 1,
                    "maximum": MAX_OUTPUT_CHARS,
                    "description": format!(
                        "Maximum number of characters to return. Default is {DEFAULT_MAX_CHARS}. Maximum is {MAX_OUTPUT_CHARS}."
                    )
                }
            },
            "required": ["path"]
        })
    }

    async fn execute(&self, _arguments: Value) -> Result<ToolExecutionResult, ToolError> {
        let _ = &self.workspace_root;
        Err(ToolError::Unavailable(
            "pdf_read execution is not implemented yet".into(),
        ))
    }
}
