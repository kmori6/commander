use async_trait::async_trait;
use reqwest::{header, redirect::Policy};
use serde::Deserialize;
use serde_json::{Value, json};
use std::net::IpAddr;
use std::time::Duration;

use crate::domain::error::tool_error::ToolError;
use crate::domain::model::tool_call::ToolPermissionMode;
use crate::domain::port::tool::Tool;

const DEFAULT_MAX_CHARS: usize = 100_000;
const MAX_CHARS: usize = 500_000;
const MAX_RESPONSE_BYTES: usize = 1_000_000;
const DEFAULT_TIMEOUT_SECONDS: u64 = 20;
const MAX_REDIRECTS: usize = 5;
const USER_AGENT: &str = "commander/0.1 web_fetch";

#[derive(Debug, Clone)]
pub struct WebFetchTool {
    client: reqwest::Client,
}

impl WebFetchTool {
    pub fn new() -> Result<Self, ToolError> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(DEFAULT_TIMEOUT_SECONDS))
            .redirect(Policy::limited(MAX_REDIRECTS))
            .user_agent(USER_AGENT)
            .build()
            .map_err(|err| ToolError::ExecutionFailed(err.to_string()))?;

        Ok(Self { client })
    }
}

#[derive(Debug, Deserialize)]
struct WebFetchArguments {
    url: String,
    max_chars: Option<usize>,
}

#[async_trait]
impl Tool for WebFetchTool {
    fn name(&self) -> &'static str {
        "web_fetch"
    }

    fn description(&self) -> &'static str {
        "Read one web page and return extracted text. Use when you already have the URL."
    }

    fn default_permission(&self) -> ToolPermissionMode {
        ToolPermissionMode::Allow
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "url": {
                    "type": "string",
                    "description": "Page URL to read. Must start with http:// or https://."
                },
                "max_chars": {
                    "type": "integer",
                    "description": format!(
                        "Text limit for the returned content. Default: {DEFAULT_MAX_CHARS}. Maximum: {MAX_CHARS}."
                    ),
                    "minimum": 1,
                    "maximum": MAX_CHARS
                }
            },
            "required": ["url"],
            "additionalProperties": false
        })
    }

    async fn execute(&self, arguments: Value) -> Result<Value, ToolError> {
        let args: WebFetchArguments = serde_json::from_value(arguments)
            .map_err(|err| ToolError::InvalidArguments(err.to_string()))?;

        let url = validate_url(&args.url)?;
        let max_chars = args.max_chars.unwrap_or(DEFAULT_MAX_CHARS);

        if max_chars == 0 || max_chars > MAX_CHARS {
            return Err(ToolError::InvalidArguments(format!(
                "max_chars must be between 1 and {MAX_CHARS}"
            )));
        }

        let response = self.client.get(url.clone()).send().await.map_err(|err| {
            if err.is_timeout() {
                ToolError::ExecutionFailed("web fetch timed out".to_string())
            } else {
                ToolError::ExecutionFailed(err.to_string())
            }
        })?;

        if !response.status().is_success() {
            return Err(ToolError::ExecutionFailed(format!(
                "web fetch failed: HTTP {}",
                response.status()
            )));
        }

        if let Some(length) = response.headers().get(header::CONTENT_LENGTH) {
            let length = length
                .to_str()
                .ok()
                .and_then(|value| value.parse::<usize>().ok());

            if matches!(length, Some(length) if length > MAX_RESPONSE_BYTES) {
                return Err(ToolError::ExecutionFailed(format!(
                    "response is too large; limit is {MAX_RESPONSE_BYTES} bytes"
                )));
            }
        }

        let content_type = response
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or("")
            .to_ascii_lowercase();

        let bytes = response
            .bytes()
            .await
            .map_err(|err| ToolError::ExecutionFailed(err.to_string()))?;

        if bytes.len() > MAX_RESPONSE_BYTES {
            return Err(ToolError::ExecutionFailed(format!(
                "response is too large; limit is {MAX_RESPONSE_BYTES} bytes"
            )));
        }

        let body = String::from_utf8_lossy(&bytes).to_string();
        let text = extract_text(&content_type, &body)?;
        let (content, truncated) = truncate_chars(&text, max_chars);

        Ok(json!({
            "url": url.as_str(),
            "content": content,
            "truncated": truncated,
        }))
    }
}

fn validate_url(raw_url: &str) -> Result<reqwest::Url, ToolError> {
    let raw_url = raw_url.trim();

    if raw_url.is_empty() {
        return Err(ToolError::InvalidArguments(
            "url must not be empty".to_string(),
        ));
    }

    let url =
        reqwest::Url::parse(raw_url).map_err(|err| ToolError::InvalidArguments(err.to_string()))?;

    if !matches!(url.scheme(), "http" | "https") {
        return Err(ToolError::InvalidArguments(
            "url must start with http:// or https://".to_string(),
        ));
    }

    let Some(host) = url.host_str() else {
        return Err(ToolError::InvalidArguments(
            "url must include a host".to_string(),
        ));
    };

    if host.eq_ignore_ascii_case("localhost") || host.ends_with(".localhost") {
        return Err(ToolError::InvalidArguments(
            "localhost URLs are not allowed".to_string(),
        ));
    }

    if let Ok(ip) = host.parse::<IpAddr>()
        && is_blocked_ip(ip)
    {
        return Err(ToolError::InvalidArguments(
            "private or local IP URLs are not allowed".to_string(),
        ));
    }

    Ok(url)
}

fn is_blocked_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => {
            ip.is_private()
                || ip.is_loopback()
                || ip.is_link_local()
                || ip.is_broadcast()
                || ip.is_documentation()
                || ip.is_unspecified()
        }
        IpAddr::V6(ip) => {
            ip.is_loopback()
                || ip.is_unspecified()
                || ip.is_unique_local()
                || ip.is_unicast_link_local()
        }
    }
}

fn extract_text(content_type: &str, body: &str) -> Result<String, ToolError> {
    let is_html = content_type.is_empty() || content_type.contains("text/html");
    let is_json = content_type.contains("application/json") || content_type.contains("+json");
    let is_text = content_type.starts_with("text/")
        || content_type.contains("application/xml")
        || content_type.contains("application/javascript");

    if is_html {
        html2text::from_read(body.as_bytes(), 80)
            .map(|text| text.trim().to_string())
            .map_err(|err| ToolError::ExecutionFailed(err.to_string()))
    } else if is_json {
        let pretty = serde_json::from_str::<Value>(body)
            .ok()
            .and_then(|value| serde_json::to_string_pretty(&value).ok())
            .unwrap_or_else(|| body.to_string());

        Ok(pretty)
    } else if is_text {
        Ok(body.to_string())
    } else {
        Err(ToolError::ExecutionFailed(format!(
            "unsupported content type: {content_type}"
        )))
    }
}

fn truncate_chars(text: &str, max_chars: usize) -> (String, bool) {
    let mut chars = text.chars();

    let content = chars.by_ref().take(max_chars).collect::<String>();
    let truncated = chars.next().is_some();

    if truncated {
        (format!("{content}\n... [truncated]"), true)
    } else {
        (content, false)
    }
}
