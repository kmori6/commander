use async_trait::async_trait;
use rmcp::{
    RoleClient, ServiceExt,
    model::{CallToolRequestParams, JsonObject},
    service::RunningService,
    transport::{ConfigureCommandExt, TokioChildProcess},
};
use serde_json::Value;
use std::{collections::HashSet, path::PathBuf, sync::Arc};
use tokio::process::Command;

use crate::domain::error::tool_error::ToolError;
use crate::domain::port::tool::Tool;
use crate::infrastructure::mcp::config::McpConfig;

type McpClient = RunningService<RoleClient, ()>;

pub struct McpTool {
    name: String,
    description: String,
    parameters: Value,
    original_tool_name: String,
    client: Arc<McpClient>,
}

#[async_trait]
impl Tool for McpTool {
    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn parameters(&self) -> Value {
        self.parameters.clone()
    }

    async fn execute(&self, arguments: Value) -> Result<Value, ToolError> {
        let arguments: JsonObject = match arguments {
            Value::Object(map) => map,
            Value::Null => JsonObject::new(),
            other => {
                return Err(ToolError::ExecutionFailed(format!(
                    "MCP tool arguments must be a JSON object, got {other}"
                )));
            }
        };

        // client-side MCP call (https://github.com/modelcontextprotocol/rust-sdk)
        let result = self
            .client
            .call_tool(
                CallToolRequestParams::new(self.original_tool_name.clone())
                    .with_arguments(arguments),
            )
            .await
            .map_err(|err| ToolError::ExecutionFailed(err.to_string()))?;

        serde_json::to_value(result).map_err(|err| ToolError::ExecutionFailed(err.to_string()))
    }
}

pub async fn load_mcp_tools(path: PathBuf) -> Result<Vec<Arc<dyn Tool>>, ToolError> {
    let Some(config) = McpConfig::load_optional(&path)
        .await
        .map_err(|err| ToolError::ExecutionFailed(err.to_string()))?
    else {
        return Ok(Vec::new());
    };

    let mut loaded_tools: Vec<Arc<dyn Tool>> = Vec::new();
    let mut names = HashSet::new();

    for (server_name, server_config) in config.servers {
        // rmcp stdio client: TokioChildProcess -> serve -> list_all_tools
        let transport =
            TokioChildProcess::new(Command::new(&server_config.command).configure(|cmd| {
                cmd.args(&server_config.args).envs(&server_config.env);
            }))
            .map_err(|err| ToolError::ExecutionFailed(err.to_string()))?;

        let client = ()
            .serve(transport)
            .await
            .map_err(|err| ToolError::ExecutionFailed(err.to_string()))?;
        let client = Arc::new(client);

        let tools = client
            .list_all_tools()
            .await
            .map_err(|err| ToolError::ExecutionFailed(err.to_string()))?;

        for tool in tools {
            let original_tool_name = tool.name.trim().to_string();
            if original_tool_name.is_empty() {
                continue;
            }

            let name = tool_name(&server_name, &original_tool_name);

            // if there are duplicate tool names, we skip them to avoid conflicts.
            if !names.insert(name.clone()) {
                log::warn!("duplicate MCP tool name skipped: {name}");
                continue;
            }

            let description = tool
                .description
                .as_deref()
                .map(str::to_string)
                .unwrap_or_else(|| format!("MCP tool from {server_name} server"));

            loaded_tools.push(Arc::new(McpTool {
                name,
                description: format!("[MCP:{server_name}] {description}"),
                parameters: tool.schema_as_json_value(),
                original_tool_name,
                client: Arc::clone(&client),
            }));
        }
    }

    Ok(loaded_tools)
}

fn tool_name(server: &str, tool: &str) -> String {
    format!("mcp__{}__{}", sanitize_name(server), sanitize_name(tool))
}

fn sanitize_name(value: &str) -> String {
    let mut out = String::new();

    for ch in value.trim().chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
            out.push(ch.to_ascii_lowercase());
        } else {
            out.push('_');
        }
    }

    let out = out.trim_matches('_').to_string();
    if out.is_empty() {
        "unnamed".to_string()
    } else {
        out
    }
}
