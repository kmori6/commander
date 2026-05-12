use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use tokio::fs;

use crate::domain::error::tool_executor_error::ToolExecutorError;
use crate::domain::model::message::{MessageContent, Role};
use crate::domain::model::tool_call::ToolPermissionMode;
use crate::domain::port::llm_provider::{LlmMessage, LlmProvider, LlmRequest};
use crate::domain::port::tool::Tool;
use crate::domain::util::data_uri::encode_data_uri;
use crate::infrastructure::llm::bedrock_llm_provider::BedrockLlmProvider;

const MODEL: &str = "global.anthropic.claude-sonnet-4-6";
const MAX_SOURCE_BYTES: u64 = 1_000_000_000;

#[derive(Debug, Clone)]
pub struct VisualInspectTool {
    workspace_root: PathBuf,
    llm_provider: BedrockLlmProvider,
}

impl VisualInspectTool {
    pub fn new(workspace_root: PathBuf, llm_provider: BedrockLlmProvider) -> Self {
        Self {
            workspace_root,
            llm_provider,
        }
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
struct VisualInspectArguments {
    path: String,
    instruction: String,
}

#[async_trait]
impl Tool for VisualInspectTool {
    fn name(&self) -> &'static str {
        "visual_inspect"
    }

    fn description(&self) -> &'static str {
        "Inspect a local image or PDF with a vision-capable model. Use when visual or layout information is needed from screenshots, rendered slides, charts, diagrams, scanned documents, or PDFs."
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
                    "description": "Local image or PDF path to inspect. Prefer a path relative to the workspace root."
                },
                "instruction": {
                    "type": "string",
                    "description": "What to inspect, extract, verify, summarize, or answer about the visual content."
                }
            },
            "required": ["path", "instruction"],
            "additionalProperties": false
        })
    }

    async fn execute(&self, arguments: Value) -> Result<Value, ToolExecutorError> {
        let args: VisualInspectArguments = serde_json::from_value(arguments)
            .map_err(|err| ToolExecutorError::InvalidArguments(err.to_string()))?;

        let path = self.resolve_path(&args.path).await?;
        let (mime_type, source_kind) = infer_source(&path)?;

        let metadata = fs::metadata(&path)
            .await
            .map_err(|err| ToolExecutorError::ExecutionFailed(err.to_string()))?;

        if !metadata.is_file() {
            return Err(ToolExecutorError::ExecutionFailed(format!(
                "path is not a file: {}",
                args.path
            )));
        }

        if metadata.len() == 0 {
            return Err(ToolExecutorError::ExecutionFailed(
                "visual source file is empty".to_string(),
            ));
        }

        if metadata.len() > MAX_SOURCE_BYTES {
            return Err(ToolExecutorError::ExecutionFailed(format!(
                "visual source file is too large; limit is {MAX_SOURCE_BYTES} bytes"
            )));
        }

        let bytes = fs::read(&path)
            .await
            .map_err(|err| ToolExecutorError::ExecutionFailed(err.to_string()))?;
        let data_uri = encode_data_uri(mime_type, &bytes);

        let visual_content = match source_kind {
            SourceKind::Image => MessageContent::input_image(data_uri),
            SourceKind::Pdf => MessageContent::input_file(filename(&path), data_uri),
        };

        let request = LlmRequest::new(
            MODEL,
            vec![
                LlmMessage::system_text(
                    "You are a visual inspection tool. Answer the user's instruction using only the provided image or document. Be concise, factual, and preserve important visible text when extracting.",
                ),
                LlmMessage::new(
                    Role::User,
                    vec![
                        MessageContent::input_text(args.instruction.clone()),
                        visual_content,
                    ],
                ),
            ],
        );

        let response = self
            .llm_provider
            .respond(request)
            .await
            .map_err(|err| ToolExecutorError::ExecutionFailed(err.to_string()))?;

        let content = response.output_text("\n").trim().to_string();
        if content.is_empty() {
            return Err(ToolExecutorError::ExecutionFailed(
                "visual inspection returned empty content".to_string(),
            ));
        }

        Ok(json!({
            "path": args.path,
            "content": content
        }))
    }
}

#[derive(Debug, Clone, Copy)]
enum SourceKind {
    Image,
    Pdf,
}

fn infer_source(path: &Path) -> Result<(&'static str, SourceKind), ToolExecutorError> {
    match path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_ascii_lowercase())
        .as_deref()
    {
        Some("png") => Ok(("image/png", SourceKind::Image)),
        Some("jpg") | Some("jpeg") => Ok(("image/jpeg", SourceKind::Image)),
        Some("gif") => Ok(("image/gif", SourceKind::Image)),
        Some("webp") => Ok(("image/webp", SourceKind::Image)),
        Some("pdf") => Ok(("application/pdf", SourceKind::Pdf)),
        Some(other) => Err(ToolExecutorError::InvalidArguments(format!(
            "unsupported visual source format: {other}"
        ))),
        None => Err(ToolExecutorError::InvalidArguments(
            "file extension is required".to_string(),
        )),
    }
}

fn filename(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("document.pdf")
        .to_string()
}
