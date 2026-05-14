use std::path::{Path, PathBuf};
use std::time::Duration;

use async_trait::async_trait;
use reqwest::multipart::{Form, Part};
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::fs;

use crate::domain::error::tool_executor_error::ToolExecutorError;
use crate::domain::model::tool_call::ToolPermissionMode;
use crate::domain::port::tool::Tool;

const MODEL: &str = "deepdml/faster-whisper-large-v3-turbo-ct2";
const DEFAULT_BASE_URL: &str = "http://127.0.0.1:8000";
const TIMEOUT_SECONDS: u64 = 300;

#[derive(Clone)]
pub struct TranscribeTool {
    workspace_root: PathBuf,
    client: reqwest::Client,
    base_url: String,
}

impl TranscribeTool {
    pub fn new(workspace_root: impl Into<PathBuf>) -> Result<Self, ToolExecutorError> {
        let workspace_root = std::fs::canonicalize(workspace_root.into())
            .map_err(|err| ToolExecutorError::ExecutionFailed(err.to_string()))?;

        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(TIMEOUT_SECONDS))
            .build()
            .map_err(|err| ToolExecutorError::ExecutionFailed(err.to_string()))?;

        Ok(Self {
            workspace_root,
            client,
            base_url: DEFAULT_BASE_URL.to_string(),
        })
    }

    async fn resolve_path(&self, path: &str) -> Result<PathBuf, ToolExecutorError> {
        let path = path.trim();

        if path.is_empty() {
            return Err(ToolExecutorError::InvalidArguments(
                "audio_path must not be empty".to_string(),
            ));
        }

        let requested = Path::new(path);
        let joined = if requested.is_absolute() {
            requested.to_path_buf()
        } else {
            self.workspace_root.join(requested)
        };

        let resolved = fs::canonicalize(joined)
            .await
            .map_err(|err| ToolExecutorError::ExecutionFailed(err.to_string()))?;

        if !resolved.starts_with(&self.workspace_root) {
            return Err(ToolExecutorError::ExecutionFailed(format!(
                "audio_path is outside workspace: {path}"
            )));
        }

        Ok(resolved)
    }
}

#[derive(Debug, Deserialize)]
struct TranscribeArguments {
    audio_path: String,
    language: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TranscriptionResponse {
    language: Option<String>,
    segments: Option<Vec<TranscriptionSegment>>,
}

#[derive(Debug, Deserialize)]
struct TranscriptionSegment {
    start: f64,
    end: f64,
    text: String,
}

#[async_trait]
impl Tool for TranscribeTool {
    fn name(&self) -> &'static str {
        "transcribe"
    }

    fn description(&self) -> &'static str {
        "Transcribe a local audio file into timestamped speech segments."
    }

    fn default_permission(&self) -> ToolPermissionMode {
        ToolPermissionMode::Allow
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "audio_path": {
                    "type": "string",
                    "description": "Path to an audio file inside the workspace. Prefer a path relative to the workspace root."
                },
                "language": {
                    "type": "string",
                    "description": "Optional ISO-639-1 language code such as ja or en. Omit to auto-detect."
                }
            },
            "required": ["audio_path"],
            "additionalProperties": false
        })
    }

    async fn execute(&self, arguments: Value) -> Result<Value, ToolExecutorError> {
        let args = serde_json::from_value::<TranscribeArguments>(arguments)
            .map_err(|err| ToolExecutorError::InvalidArguments(err.to_string()))?;

        let language = args.language.and_then(|value| {
            let value = value.trim().to_string();
            (!value.is_empty()).then_some(value)
        });

        let path = self.resolve_path(&args.audio_path).await?;
        let metadata = fs::metadata(&path)
            .await
            .map_err(|err| ToolExecutorError::ExecutionFailed(err.to_string()))?;

        if !metadata.is_file() {
            return Err(ToolExecutorError::ExecutionFailed(format!(
                "audio_path is not a file: {}",
                args.audio_path
            )));
        }

        let bytes = fs::read(&path)
            .await
            .map_err(|err| ToolExecutorError::ExecutionFailed(err.to_string()))?;

        let file_name = path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("audio")
            .to_string();

        let mut form = Form::new()
            .part("file", Part::bytes(bytes).file_name(file_name))
            .text("model", MODEL)
            .text("response_format", "verbose_json");

        if let Some(language) = &language {
            form = form.text("language", language.clone());
        }

        let url = format!(
            "{}/v1/audio/transcriptions",
            self.base_url.trim_end_matches('/')
        );

        let response = self
            .client
            .post(url)
            .multipart(form)
            .send()
            .await
            .map_err(|err| ToolExecutorError::ExecutionFailed(err.to_string()))?;

        let status = response.status();
        let body = response
            .text()
            .await
            .map_err(|err| ToolExecutorError::ExecutionFailed(err.to_string()))?;

        if !status.is_success() {
            return Err(ToolExecutorError::ExecutionFailed(format!(
                "transcription failed: HTTP {status}: {body}"
            )));
        }

        let transcription = serde_json::from_str::<TranscriptionResponse>(&body)
            .map_err(|err| ToolExecutorError::ExecutionFailed(err.to_string()))?;

        let segments = transcription
            .segments
            .unwrap_or_default()
            .into_iter()
            .map(|segment| {
                json!({
                    "start": segment.start,
                    "end": segment.end,
                    "text": segment.text,
                })
            })
            .collect::<Vec<_>>();

        Ok(json!({
            "language": transcription.language.or(language),
            "segments": segments,
        }))
    }
}
