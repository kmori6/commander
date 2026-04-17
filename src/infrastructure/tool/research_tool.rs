use crate::application::error::llm_client_error::LlmClientError;
use crate::domain::error::tool_error::ToolError;
use crate::domain::model::message::Message;
use crate::domain::model::role::Role;
use crate::domain::model::tool::ToolExecutionResult;
use crate::domain::port::llm_provider::LlmProvider;
use crate::domain::port::tool::Tool;
use async_recursion::async_recursion;
use async_trait::async_trait;
use serde_json::{Value, json};
use std::time::Duration;
use tavily::{SearchRequest, SearchResult, Tavily, TavilyError};

const DEFAULT_MODEL: &str = "global.anthropic.claude-sonnet-4-6";
const DEFAULT_DEPTH: u32 = 1;
const DEFAULT_BREADTH: u32 = 3;
const MAX_DEPTH: u32 = 3;
const MAX_BREADTH: u32 = 5;
const SEARCH_TIMEOUT_SECS: u64 = 30;
const SEARCH_MAX_RESULTS: i32 = 10;
const SEARCH_DAYS: i32 = 1;

pub struct ResearchTool<L> {
    api_key: String,
    client: Tavily,
    llm_provider: L,
    model: String,
}

impl<L> ResearchTool<L> {
    pub fn new(
        api_key: impl Into<String>,
        llm_provider: L,
        model: impl Into<String>,
    ) -> Result<Self, ToolError> {
        let api_key = api_key.into();
        let api_key = api_key.trim();
        if api_key.is_empty() {
            return Err(ToolError::Unavailable(
                "TAVILY_API_KEY must not be empty".into(),
            ));
        }

        let model = model.into();
        let model = model.trim();
        if model.is_empty() {
            return Err(ToolError::Unavailable(
                "research model must not be empty".into(),
            ));
        }

        let client = Tavily::builder(api_key)
            .timeout(Duration::from_secs(SEARCH_TIMEOUT_SECS))
            .build()
            .map_err(map_tavily_error)?;

        Ok(Self {
            api_key: api_key.to_string(),
            client,
            llm_provider,
            model: model.to_string(),
        })
    }

    pub fn from_env(llm_provider: L) -> Result<Self, ToolError> {
        let api_key = std::env::var("TAVILY_API_KEY")
            .map_err(|_| ToolError::Unavailable("TAVILY_API_KEY is not set".into()))?;
        let model = std::env::var("RESEARCH_TOOL_MODEL")
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| DEFAULT_MODEL.to_string());

        Self::new(api_key, llm_provider, model)
    }
}

impl<L: LlmProvider> ResearchTool<L> {
    #[async_recursion]
    async fn research(
        &self,
        query: &str,
        depth: u32,
        breadth: u32,
    ) -> Result<Vec<SearchResult>, ToolError> {
        let expanded_queries = self.expand_queries(query, breadth).await?;

        let mut search_results = Vec::new();
        for expanded_query in expanded_queries {
            let response = self
                .client
                .call(&self.build_search_request(&expanded_query))
                .await
                .map_err(map_tavily_error)?;
            search_results.extend(response.results);
        }

        let sub_queries = self
            .extract_sub_queries(query, &search_results, breadth)
            .await?;

        if depth == 0 {
            return Ok(search_results);
        }

        for sub_query in sub_queries {
            let sub_results = self.research(&sub_query, depth - 1, breadth).await?;
            search_results.extend(sub_results);
        }

        Ok(search_results)
    }

    async fn expand_queries(&self, query: &str, breadth: u32) -> Result<Vec<String>, ToolError> {
        let instructions = "You are a research assistant. Your task is to generate search queries.";
        let user_content = format!(
            r#"Generate {breadth} diverse search queries to thoroughly research the following topic.

Topic: {query}

Return ONLY a JSON array of strings. Example:
["query 1", "query 2", "query 3"]

Do not include any explanation or extra text."#
        );

        self.request_query_list(instructions, user_content, "queries")
            .await
    }

    async fn extract_sub_queries(
        &self,
        original_query: &str,
        results: &[SearchResult],
        breadth: u32,
    ) -> Result<Vec<String>, ToolError> {
        let articles_context = results
            .iter()
            .enumerate()
            .map(|(index, result)| format!("[{}] {}\n{}", index + 1, result.title, result.content))
            .collect::<Vec<_>>()
            .join("\n\n");

        let instructions = "You are a research assistant. Your task is to identify unexplored sub-topics from collected articles.";
        let user_content = format!(
            r#"You are researching the following topic:
{original_query}

You have collected these articles:
{articles_context}

Based on the articles above, identify the most important sub-topics or angles that were mentioned but not fully covered.

Return ONLY a JSON array of at most {breadth} search query strings. Example:
["sub-topic query 1", "sub-topic query 2"]

Do not include any explanation or extra text."#
        );

        self.request_query_list(instructions, user_content, "sub-queries")
            .await
    }

    async fn request_query_list(
        &self,
        instructions: &str,
        user_content: String,
        label: &str,
    ) -> Result<Vec<String>, ToolError> {
        let messages = vec![
            Message::text(Role::System, instructions),
            Message::text(Role::User, user_content),
        ];

        let response_text = self
            .llm_provider
            .response(messages, &self.model)
            .await
            .map_err(map_llm_error)?;

        let queries: Vec<String> = serde_json::from_str(response_text.trim()).map_err(|err| {
            ToolError::ExecutionFailed(format!("failed to parse {label} JSON: {err}"))
        })?;

        Ok(queries
            .into_iter()
            .map(|query| query.trim().to_string())
            .filter(|query| !query.is_empty())
            .collect())
    }

    fn build_search_request(&self, query: &str) -> SearchRequest {
        SearchRequest::new(&self.api_key, query)
            .topic("news")
            .search_depth("advanced")
            .max_results(SEARCH_MAX_RESULTS)
            .days(SEARCH_DAYS)
            .include_raw_content(false)
    }
}

#[async_trait]
impl<L: LlmProvider> Tool for ResearchTool<L> {
    fn name(&self) -> &str {
        "research"
    }

    fn description(&self) -> &str {
        "Perform recursive news research by expanding the query with the LLM, searching Tavily, and following deeper sub-topics."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "The research topic to investigate."
                },
                "depth": {
                    "type": "integer",
                    "description": format!("How many recursive sub-query levels to explore. Default is {DEFAULT_DEPTH}. Maximum is {MAX_DEPTH}.")
                },
                "breadth": {
                    "type": "integer",
                    "description": format!("How many expanded queries and sub-queries to request from the LLM at each step. Default is {DEFAULT_BREADTH}. Maximum is {MAX_BREADTH}.")
                }
            },
            "required": ["query"]
        })
    }

    async fn execute(&self, arguments: Value) -> Result<ToolExecutionResult, ToolError> {
        let query = arguments
            .get("query")
            .and_then(|value| value.as_str())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| ToolError::InvalidArguments("missing or invalid 'query'".into()))?;

        let depth = parse_u32_argument(&arguments, "depth")?.unwrap_or(DEFAULT_DEPTH);
        if depth > MAX_DEPTH {
            return Err(ToolError::InvalidArguments(format!(
                "'depth' must be between 0 and {MAX_DEPTH}"
            )));
        }

        let breadth = parse_u32_argument(&arguments, "breadth")?.unwrap_or(DEFAULT_BREADTH);
        if breadth == 0 || breadth > MAX_BREADTH {
            return Err(ToolError::InvalidArguments(format!(
                "'breadth' must be between 1 and {MAX_BREADTH}"
            )));
        }

        let results = self.research(query, depth, breadth).await?;

        Ok(ToolExecutionResult::success(json!({
            "query": query,
            "depth": depth,
            "breadth": breadth,
            "total_results": results.len(),
            "results": results.into_iter().map(search_result_to_json).collect::<Vec<_>>(),
        })))
    }
}

fn parse_u32_argument(arguments: &Value, field_name: &str) -> Result<Option<u32>, ToolError> {
    let Some(value) = arguments.get(field_name) else {
        return Ok(None);
    };

    let value = value.as_u64().ok_or_else(|| {
        ToolError::InvalidArguments(format!("'{field_name}' must be a non-negative integer"))
    })?;

    let value = u32::try_from(value).map_err(|_| {
        ToolError::InvalidArguments(format!("'{field_name}' is out of supported range"))
    })?;

    Ok(Some(value))
}

fn search_result_to_json(result: SearchResult) -> Value {
    json!({
        "title": result.title,
        "url": result.url,
        "content": result.content,
        "raw_content": result.raw_content,
        "score": result.score,
    })
}

fn map_llm_error(err: LlmClientError) -> ToolError {
    ToolError::ExecutionFailed(format!("research llm request failed: {err}"))
}

fn map_tavily_error(err: TavilyError) -> ToolError {
    match err {
        TavilyError::Configuration(msg) => {
            ToolError::Unavailable(format!("tavily configuration error: {msg}"))
        }
        TavilyError::RateLimit(msg) => ToolError::Unavailable(format!("tavily rate limit: {msg}")),
        TavilyError::Http(err) if err.is_timeout() => ToolError::Timeout,
        TavilyError::Http(err) => ToolError::Unavailable(format!("tavily http error: {err}")),
        TavilyError::Api(msg) => ToolError::ExecutionFailed(format!("tavily api error: {msg}")),
    }
}
