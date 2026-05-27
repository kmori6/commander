use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::time::Duration;

use crate::domain::error::tool_service_error::ToolServiceError;
use crate::domain::model::tool_call::ToolPermissionMode;
use crate::domain::port::tool::Tool;

const TAVILY_SEARCH_URL: &str = "https://api.tavily.com/search";
const DEFAULT_TIMEOUT_SECONDS: u64 = 30;
const DEFAULT_MAX_RESULTS: u8 = 5;
const MAX_RESULTS: u8 = 20;
const MAX_DOMAINS: usize = 20;
const MAX_ERROR_BODY_BYTES: usize = 2_000;

#[derive(Debug, Clone)]
pub struct WebSearchTool {
    api_key: String,
    client: reqwest::Client,
}

impl WebSearchTool {
    pub fn from_env() -> Result<Self, ToolServiceError> {
        let api_key = std::env::var("TAVILY_API_KEY").map_err(|_| {
            ToolServiceError::ExecutionFailed("TAVILY_API_KEY is not set".to_string())
        })?;

        Self::new(api_key)
    }

    pub fn new(api_key: impl Into<String>) -> Result<Self, ToolServiceError> {
        let api_key = api_key.into().trim().to_string();

        if api_key.is_empty() {
            return Err(ToolServiceError::ExecutionFailed(
                "TAVILY_API_KEY must not be empty".to_string(),
            ));
        }

        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(DEFAULT_TIMEOUT_SECONDS))
            .build()
            .map_err(|err| ToolServiceError::ExecutionFailed(err.to_string()))?;

        Ok(Self { api_key, client })
    }
}

#[derive(Debug, Deserialize)]
struct WebSearchArguments {
    query: String,
    max_results: Option<u8>,
    search_depth: Option<String>,
    topic: Option<String>,
    time_range: Option<String>,
    domains: Option<Vec<String>>,
}

#[derive(Debug, Serialize)]
struct TavilySearchRequest<'a> {
    query: &'a str,
    max_results: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    search_depth: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    topic: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    time_range: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    include_domains: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct TavilySearchResponse {
    query: String,
    results: Vec<TavilySearchResult>,
}

#[derive(Debug, Deserialize)]
struct TavilySearchResult {
    title: String,
    url: String,
    content: String,
}

#[async_trait]
impl Tool for WebSearchTool {
    fn name(&self) -> &'static str {
        "web_search"
    }

    fn description(&self) -> &'static str {
        "Find web pages with Tavily. Returns titles, URLs, and content excerpts for choosing sources."
    }

    fn default_permission(&self) -> ToolPermissionMode {
        ToolPermissionMode::Allow
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "What to look up on the web. Add names, dates, or site hints when they matter."
                },
                "max_results": {
                    "type": "integer",
                    "description": "Result limit. Default: 5. Maximum: 20.",
                    "minimum": 1,
                    "maximum": MAX_RESULTS
                },
                "search_depth": {
                    "type": "string",
                    "enum": ["basic", "advanced"],
                    "description": "basic is faster; advanced spends more work on harder research."
                },
                "topic": {
                    "type": "string",
                    "enum": ["general", "news", "finance"],
                    "description": "Result category. Default: general."
                },
                "time_range": {
                    "type": "string",
                    "enum": ["day", "week", "month", "year"],
                    "description": "Prefer pages published or updated in this recent window."
                },
                "domains": {
                    "type": "array",
                    "description": "Restrict results to these domains, such as docs.rs or github.com.",
                    "items": { "type": "string" },
                    "maxItems": MAX_DOMAINS
                }
            },
            "required": ["query"],
            "additionalProperties": false
        })
    }

    async fn execute(&self, arguments: Value) -> Result<Value, ToolServiceError> {
        let args: WebSearchArguments = serde_json::from_value(arguments)
            .map_err(|err| ToolServiceError::InvalidArguments(err.to_string()))?;

        let query = args.query.trim();

        if query.is_empty() {
            return Err(ToolServiceError::InvalidArguments(
                "query must not be empty".to_string(),
            ));
        }

        let max_results = args.max_results.unwrap_or(DEFAULT_MAX_RESULTS);

        if max_results == 0 || max_results > MAX_RESULTS {
            return Err(ToolServiceError::InvalidArguments(format!(
                "max_results must be between 1 and {MAX_RESULTS}"
            )));
        }

        let search_depth =
            validate_enum(args.search_depth, "search_depth", &["basic", "advanced"])?;
        let topic = validate_enum(args.topic, "topic", &["general", "news", "finance"])?;
        let time_range = validate_enum(
            args.time_range,
            "time_range",
            &["day", "week", "month", "year"],
        )?;
        let domains = normalize_domains(args.domains)?;

        let request = TavilySearchRequest {
            query,
            max_results,
            search_depth,
            topic,
            time_range,
            include_domains: domains,
        };

        let response = self
            .client
            .post(TAVILY_SEARCH_URL)
            .bearer_auth(&self.api_key)
            .json(&request)
            .send()
            .await
            .map_err(|err| {
                if err.is_timeout() {
                    ToolServiceError::ExecutionFailed("Tavily search timed out".to_string())
                } else {
                    ToolServiceError::ExecutionFailed(err.to_string())
                }
            })?;

        if !response.status().is_success() {
            return Err(tavily_error(response).await);
        }

        let payload: TavilySearchResponse = response
            .json()
            .await
            .map_err(|err| ToolServiceError::ExecutionFailed(err.to_string()))?;

        let results = payload
            .results
            .into_iter()
            .map(|result| {
                json!({
                    "title": result.title,
                    "url": result.url,
                    "content": result.content,
                })
            })
            .collect::<Vec<_>>();

        Ok(json!({
            "query": payload.query,
            "results": results,
        }))
    }
}

fn validate_enum(
    value: Option<String>,
    name: &str,
    allowed: &[&str],
) -> Result<Option<String>, ToolServiceError> {
    let Some(value) = value else {
        return Ok(None);
    };

    let value = value.trim().to_ascii_lowercase();

    if value.is_empty() {
        return Ok(None);
    }

    if allowed.contains(&value.as_str()) {
        Ok(Some(value))
    } else {
        Err(ToolServiceError::InvalidArguments(format!(
            "{name} must be one of: {}",
            allowed.join(", ")
        )))
    }
}

fn normalize_domains(value: Option<Vec<String>>) -> Result<Vec<String>, ToolServiceError> {
    let mut domains = Vec::new();

    for raw in value.unwrap_or_default() {
        let raw = raw.trim();

        if raw.is_empty() {
            continue;
        }

        let candidate = if raw.starts_with("http://") || raw.starts_with("https://") {
            raw.to_string()
        } else {
            format!("https://{raw}")
        };

        let parsed = reqwest::Url::parse(&candidate)
            .map_err(|err| ToolServiceError::InvalidArguments(err.to_string()))?;

        let Some(host) = parsed.host_str() else {
            return Err(ToolServiceError::InvalidArguments(format!(
                "invalid domain: {raw}"
            )));
        };

        let domain = host.trim().to_ascii_lowercase();

        if !is_valid_domain(&domain) {
            return Err(ToolServiceError::InvalidArguments(format!(
                "invalid domain: {raw}"
            )));
        }

        if !domains.iter().any(|existing| existing == &domain) {
            domains.push(domain);
        }

        if domains.len() > MAX_DOMAINS {
            return Err(ToolServiceError::InvalidArguments(format!(
                "domains must contain at most {MAX_DOMAINS} entries"
            )));
        }
    }

    Ok(domains)
}

fn is_valid_domain(domain: &str) -> bool {
    !domain.is_empty()
        && domain.len() <= 253
        && domain.contains('.')
        && domain
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '.')
}

async fn tavily_error(response: reqwest::Response) -> ToolServiceError {
    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    let body = truncate_text(&body, MAX_ERROR_BODY_BYTES);

    ToolServiceError::ExecutionFailed(format!("Tavily search failed: HTTP {status}: {body}"))
}

fn truncate_text(text: &str, max_bytes: usize) -> String {
    if text.len() <= max_bytes {
        return text.to_string();
    }

    let mut end = max_bytes;

    while !text.is_char_boundary(end) {
        end -= 1;
    }

    format!("{}... [truncated]", &text[..end])
}
