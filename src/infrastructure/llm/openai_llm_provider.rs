use async_trait::async_trait;
use reqwest::Client;
use serde_json::{Value, json};

use crate::domain::error::llm_provider_error::LlmProviderError;
use crate::domain::model::message::{MessageContent, MessageUsage, Role};
use crate::domain::port::llm_provider::{LlmMessage, LlmProvider, LlmRequest, LlmResponse};

#[derive(Clone)]
pub struct OpenaiLlmProvider {
    client: Client,
    model: String,
    base_url: String,
    context_window: i64,
    api_key: String,
}

impl OpenaiLlmProvider {
    pub fn new(
        model: impl Into<String>,
        base_url: impl Into<String>,
        context_window: i64,
        api_key: impl Into<String>,
    ) -> Self {
        Self {
            client: Client::new(),
            model: model.into(),
            base_url: base_url.into(),
            context_window,
            api_key: api_key.into(),
        }
    }
}

#[async_trait]
impl LlmProvider for OpenaiLlmProvider {
    async fn response(&self, request: LlmRequest) -> Result<LlmResponse, LlmProviderError> {
        let mut body = json!({
            "model": self.model,
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

        let url = format!("{}/responses", self.base_url);
        let response = self
            .client
            .post(url)
            .bearer_auth(&self.api_key)
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

    fn model(&self) -> &str {
        &self.model
    }

    fn context_window(&self) -> i64 {
        self.context_window
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
