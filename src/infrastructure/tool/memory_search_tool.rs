use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::Arc;

use crate::domain::error::tool_executor_error::ToolExecutorError;
use crate::domain::model::tool_call::ToolPermissionMode;
use crate::domain::port::tool::Tool;
use crate::domain::service::memory_index_service::MemoryIndexService;

const DEFAULT_LIMIT: usize = 5;
const MAX_LIMIT: usize = 20;

#[derive(Clone)]
pub struct MemorySearchTool {
    memory_index_service: Arc<MemoryIndexService>,
}

impl MemorySearchTool {
    pub fn new(memory_index_service: Arc<MemoryIndexService>) -> Self {
        Self {
            memory_index_service,
        }
    }
}

#[derive(Debug, Deserialize)]
struct MemorySearchArguments {
    query: String,
    limit: Option<usize>,
}

#[async_trait]
impl Tool for MemorySearchTool {
    fn name(&self) -> &'static str {
        "memory_search"
    }

    fn description(&self) -> &'static str {
        "Search past journal notes by meaning. Use for prior work, decisions, and saved context."
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
                    "description": "What to look for in saved journal notes."
                },
                "limit": {
                    "type": "integer",
                    "description": "Result limit. Default: 5. Maximum: 20.",
                    "minimum": 1,
                    "maximum": MAX_LIMIT
                }
            },
            "required": ["query"],
            "additionalProperties": false
        })
    }

    async fn execute(&self, arguments: Value) -> Result<Value, ToolExecutorError> {
        let args: MemorySearchArguments = serde_json::from_value(arguments)
            .map_err(|err| ToolExecutorError::InvalidArguments(err.to_string()))?;

        let query = args.query.trim();
        if query.is_empty() {
            return Err(ToolExecutorError::InvalidArguments(
                "query must not be empty".to_string(),
            ));
        }

        let limit = args.limit.unwrap_or(DEFAULT_LIMIT);
        if limit == 0 || limit > MAX_LIMIT {
            return Err(ToolExecutorError::InvalidArguments(format!(
                "limit must be between 1 and {MAX_LIMIT}"
            )));
        }

        let results = self
            .memory_index_service
            .search(query, limit)
            .await
            .map_err(|err| {
                ToolExecutorError::ExecutionFailed(format!("failed to search memory: {err}"))
            })?;

        let results = results
            .into_iter()
            .map(|result| {
                json!({
                    "path": result.path,
                    "chunk_index": result.chunk_index,
                    "content": result.content,
                })
            })
            .collect::<Vec<_>>();

        Ok(json!({
            "results": results
        }))
    }
}
