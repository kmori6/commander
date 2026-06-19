use std::{path::PathBuf, sync::Arc};

use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::fs;
use tokio::sync::RwLock;

use crate::domain::error::llm_provider_error::LlmProviderError;
use crate::domain::model::llm::Llm;
use crate::domain::model::message::{MessageContent, MessageUsage, Role};
use crate::domain::port::llm_provider::{LlmMessage, LlmProvider, LlmRequest, LlmResponse};

const EXAMPLE_MODEL_CONFIG_PATH: &str = "config/models.json";
const OPENAI_API_KEY_ENV: &str = "OPENAI_API_KEY";

#[derive(Debug, Clone)]
struct ModelConfigPaths {
    example_path: PathBuf,
    user_path: PathBuf,
}

impl ModelConfigPaths {
    fn new(user_path: PathBuf) -> Self {
        Self {
            example_path: PathBuf::from(EXAMPLE_MODEL_CONFIG_PATH),
            user_path,
        }
    }

    fn path(&self) -> PathBuf {
        self.user_path.clone()
    }

    async fn ensure_user_config(&self) -> Result<(), LlmProviderError> {
        if fs::try_exists(&self.user_path).await.map_err(|err| {
            LlmProviderError::Unexpected(format!(
                "failed to check model config {}: {err}",
                self.user_path.display()
            ))
        })? {
            return Ok(());
        }

        if let Some(parent) = self.user_path.parent() {
            fs::create_dir_all(parent).await.map_err(|err| {
                LlmProviderError::Unexpected(format!(
                    "failed to create model config directory {}: {err}",
                    parent.display()
                ))
            })?;
        }

        fs::copy(&self.example_path, &self.user_path)
            .await
            .map_err(|err| {
                LlmProviderError::Unexpected(format!(
                    "failed to initialize model config from {} to {}: {err}",
                    self.example_path.display(),
                    self.user_path.display()
                ))
            })?;

        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LlmConfig {
    default_model: String,
    models: Vec<Llm>,
}

impl LlmConfig {
    fn current_model(&self) -> Result<Llm, String> {
        let model = self.default_model.trim();
        if model.is_empty() {
            return Err("default_model must not be empty".to_string());
        }

        let mut llm = self
            .models
            .iter()
            .find(|llm| llm.model == model)
            .cloned()
            .ok_or_else(|| format!("unknown default model: {model}"))?;

        llm.model = llm.model.trim().to_string();
        llm.base_url = llm.base_url.trim().to_string();
        Self::validate_llm(&llm)?;

        Ok(llm)
    }

    fn list_llms(&self) -> Result<Vec<Llm>, String> {
        for llm in &self.models {
            Self::validate_llm(llm)?;
        }

        Ok(self.models.clone())
    }

    fn select_default_model(&mut self, model: &str) -> Result<Llm, String> {
        let model = model.trim();
        if model.is_empty() {
            return Err("model must not be empty".to_string());
        }

        let llm = self
            .models
            .iter()
            .find(|llm| llm.model == model)
            .cloned()
            .ok_or_else(|| format!("unknown model: {model}"))?;

        Self::validate_llm(&llm)?;
        self.default_model = llm.model.clone();

        Ok(llm)
    }

    fn validate(&self) -> Result<(), String> {
        self.current_model()?;

        for llm in &self.models {
            Self::validate_llm(llm)?;
        }

        Ok(())
    }

    fn validate_llm(llm: &Llm) -> Result<(), String> {
        if llm.model.trim().is_empty() {
            return Err("llm model must not be empty".to_string());
        }

        if llm.base_url.trim().is_empty() {
            return Err("llm base_url must not be empty".to_string());
        }

        if llm.context_window <= 0 {
            return Err("llm context_window must be positive".to_string());
        }

        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct OpenaiLlmProvider {
    client: Client,
    config: Arc<RwLock<LlmConfig>>,
    paths: ModelConfigPaths,
}

impl OpenaiLlmProvider {
    pub async fn from_config_path(user_path: impl Into<PathBuf>) -> Result<Self, LlmProviderError> {
        let paths = ModelConfigPaths::new(user_path.into());
        paths.ensure_user_config().await?;

        let config = load_config(&paths).await?;
        config.validate().map_err(LlmProviderError::Unexpected)?;

        Ok(Self {
            client: Client::new(),
            config: Arc::new(RwLock::new(config)),
            paths,
        })
    }

    pub async fn list_models(&self) -> Vec<Llm> {
        self.config
            .read()
            .await
            .list_llms()
            .expect("llm config is validated at load time")
    }

    pub async fn select_model(&self, model: &str) -> Result<Llm, LlmProviderError> {
        let (next_config, selected_model) = {
            let config = self.config.read().await;
            let mut next_config = config.clone();

            let selected_model = next_config
                .select_default_model(model)
                .map_err(LlmProviderError::RequestBuild)?;

            (next_config, selected_model)
        };

        save_config(&self.paths, &next_config).await?;
        *self.config.write().await = next_config;

        Ok(selected_model)
    }
}

#[async_trait]
impl LlmProvider for OpenaiLlmProvider {
    async fn response(&self, request: LlmRequest) -> Result<LlmResponse, LlmProviderError> {
        let config = self.config.read().await;
        let llm = config
            .current_model()
            .map_err(LlmProviderError::RequestBuild)?;

        let mut body = json!({
            "model": llm.model,
            "input": build_input(&request.messages)?,
        });

        if !request.tools.is_empty() {
            body["tools"] = Value::Array(
                request
                    .tools
                    .iter()
                    .map(|tool| {
                        json!({
                            "type": "function",
                            "name": tool.name,
                            "description": tool.description,
                            "parameters": tool.parameters,
                        })
                    })
                    .collect(),
            );
        }

        let api_key = std::env::var(OPENAI_API_KEY_ENV).unwrap_or_else(|_| "none".to_string());
        let url = format!("{}/responses", llm.base_url.trim_end_matches('/'));
        let response = self
            .client
            .post(url)
            .bearer_auth(api_key)
            .json(&body)
            .send()
            .await
            .map_err(|err| LlmProviderError::ApiCall(format!("OpenAI request failed: {err}")))?;

        let status = response.status();
        let text = response.text().await.map_err(|err| {
            LlmProviderError::ApiCall(format!("OpenAI response read failed: {err}"))
        })?;

        if !status.is_success() {
            return Err(LlmProviderError::ApiCall(format!(
                "OpenAI response error: status={status}, body={text}"
            )));
        }

        let value: Value = serde_json::from_str(&text).map_err(|err| {
            LlmProviderError::ResponseParse(format!("invalid OpenAI response JSON: {err}"))
        })?;

        let message = parse_output(&value)?;
        let usage = parse_usage(&value);

        Ok(LlmResponse { message, usage })
    }

    async fn context_window(&self) -> Result<i64, LlmProviderError> {
        let config = self.config.read().await;
        let llm = config
            .current_model()
            .map_err(LlmProviderError::RequestBuild)?;

        Ok(llm.context_window)
    }

    async fn current_model_id(&self) -> Result<String, LlmProviderError> {
        let config = self.config.read().await;
        let llm = config
            .current_model()
            .map_err(LlmProviderError::RequestBuild)?;

        Ok(llm.model)
    }
}

fn build_input(messages: &[LlmMessage]) -> Result<Value, LlmProviderError> {
    let mut input = Vec::new();

    for message in messages {
        let mut content = Vec::new();

        for item in &message.contents {
            if !item.fits_role(message.role) {
                return Err(LlmProviderError::RequestBuild(
                    "message content does not fit role".to_string(),
                ));
            }

            match item {
                MessageContent::InputText { text } | MessageContent::OutputText { text } => {
                    let content_type = if message.role == Role::Assistant {
                        "output_text"
                    } else {
                        "input_text"
                    };

                    content.push(json!({
                        "type": content_type,
                        "text": text,
                    }));
                }
                MessageContent::InputImage { image_url } => {
                    content.push(json!({
                        "type": "input_image",
                        "image_url": image_url,
                        "detail": "auto",
                    }));
                }
                MessageContent::InputFile {
                    filename,
                    file_data,
                } => {
                    content.push(json!({
                        "type": "input_file",
                        "filename": filename,
                        "file_data": file_data,
                    }));
                }
                MessageContent::ToolCall {
                    call_id,
                    tool_name,
                    arguments,
                } => {
                    input.push(json!({
                        "type": "function_call",
                        "call_id": call_id,
                        "name": tool_name,
                        "arguments": serde_json::to_string(arguments).map_err(|err| {
                            LlmProviderError::RequestBuild(format!(
                                "failed to encode tool call arguments: {err}"
                            ))
                        })?,
                    }));
                }
                MessageContent::ToolCallOutput {
                    call_id, output, ..
                } => {
                    input.push(json!({
                        "type": "function_call_output",
                        "call_id": call_id,
                        "output": serde_json::to_string(output).map_err(|err| {
                            LlmProviderError::RequestBuild(format!(
                                "failed to encode tool call output: {err}"
                            ))
                        })?,
                    }));
                }
            }
        }

        if content.is_empty() {
            return Err(LlmProviderError::RequestBuild(
                "message must have at least one content item".to_string(),
            ));
        }
    }

    Ok(Value::Array(input))
}

fn parse_output(value: &Value) -> Result<LlmMessage, LlmProviderError> {
    let mut contents = Vec::new();

    let output = value["output"].as_array().ok_or_else(|| {
        LlmProviderError::ResponseParse("OpenAI response missing output array".to_string())
    })?;

    for item in output {
        match item["type"].as_str() {
            Some("message") => {
                if let Some(parts) = item["content"].as_array() {
                    for part in parts {
                        match part["type"].as_str() {
                            Some("output_text") => {
                                if let Some(text) = part["text"].as_str() {
                                    contents.push(MessageContent::output_text(text));
                                }
                            }
                            Some("refusal") => {
                                if let Some(text) = part["refusal"].as_str() {
                                    contents.push(MessageContent::output_text(text));
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
            Some("function_call") => {
                let call_id = item["call_id"].as_str().ok_or_else(|| {
                    LlmProviderError::ResponseParse(
                        "OpenAI function_call missing call_id".to_string(),
                    )
                })?;

                let name = item["name"].as_str().ok_or_else(|| {
                    LlmProviderError::ResponseParse("OpenAI function_call missing name".to_string())
                })?;

                let arguments = item["arguments"].as_str().unwrap_or("{}");
                let arguments = serde_json::from_str(arguments).map_err(|err| {
                    LlmProviderError::ResponseParse(format!(
                        "invalid OpenAI function_call arguments: {err}"
                    ))
                })?;

                contents.push(MessageContent::ToolCall {
                    call_id: call_id.to_string(),
                    tool_name: name.to_string(),
                    arguments,
                });
            }
            _ => {}
        }
    }

    if contents.is_empty() {
        return Err(LlmProviderError::ResponseParse(
            "LLM response has no valid message content".to_string(),
        ));
    }

    Ok(LlmMessage::new(Role::Assistant, contents))
}

fn parse_usage(value: &Value) -> MessageUsage {
    let usage = &value["usage"];

    MessageUsage {
        input_tokens: usage["input_tokens"].as_i64().unwrap_or_default(),
        output_tokens: usage["output_tokens"].as_i64().unwrap_or_default(),
        cache_read_tokens: usage["input_tokens_details"]["cached_tokens"]
            .as_i64()
            .unwrap_or_default(),
        cache_write_tokens: 0,
    }
}

async fn load_config(paths: &ModelConfigPaths) -> Result<LlmConfig, LlmProviderError> {
    let path = paths.path();
    let content = fs::read_to_string(&path).await.map_err(|err| {
        LlmProviderError::Unexpected(format!(
            "failed to read model config {}: {err}",
            path.display()
        ))
    })?;

    serde_json::from_str(&content).map_err(|err| {
        LlmProviderError::Unexpected(format!(
            "failed to parse model config {}: {err}",
            path.display()
        ))
    })
}

async fn save_config(paths: &ModelConfigPaths, config: &LlmConfig) -> Result<(), LlmProviderError> {
    let path = paths.path();

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).await.map_err(|err| {
            LlmProviderError::Unexpected(format!(
                "failed to create model config directory {}: {err}",
                parent.display()
            ))
        })?;
    }

    let content = serde_json::to_string_pretty(config).map_err(|err| {
        LlmProviderError::Unexpected(format!("failed to serialize model config: {err}"))
    })?;

    fs::write(&path, format!("{content}\n"))
        .await
        .map_err(|err| {
            LlmProviderError::Unexpected(format!(
                "failed to write model config {}: {err}",
                path.display()
            ))
        })
}
