use rmcp::{
    RoleClient, ServiceExt,
    model::{CallToolRequestParams, JsonObject, Tool as RmcpTool},
    service::RunningService,
    transport::{ConfigureCommandExt, TokioChildProcess},
};
use serde_json::Value;
use std::{collections::HashMap, path::PathBuf, sync::Arc};
use tokio::process::Command;

use crate::domain::error::tool_error::ToolError;

use super::config::{McpConfig, McpServerConfig};

type McpClient = RunningService<RoleClient, ()>;

#[derive(Debug, Clone)]
pub struct DiscoveredMcpTool {
    pub exposed_name: String,
    pub description: String,
    pub parameters: Value,
}

#[derive(Debug, Clone)]
struct McpToolRef {
    server_name: String,
    original_tool_name: String,
}

pub struct McpManager {
    clients: HashMap<String, McpClient>,
    tools: Vec<DiscoveredMcpTool>,
    tool_index: HashMap<String, McpToolRef>,
}

impl McpManager {
    pub async fn from_config_path(path: PathBuf) -> Result<Option<Arc<Self>>, ToolError> {
        let Some(config) = McpConfig::load_optional(&path)
            .await
            .map_err(|err| ToolError::ExecutionFailed(err.to_string()))?
        else {
            return Ok(None);
        };

        if config.mcp_servers.is_empty() {
            return Ok(None);
        }

        let manager = Self::connect_all(config).await?;

        if manager.tools.is_empty() {
            return Ok(None);
        }

        Ok(Some(Arc::new(manager)))
    }

    async fn connect_all(config: McpConfig) -> Result<Self, ToolError> {
        let mut clients = HashMap::new();
        let mut tools = Vec::new();
        let mut tool_index = HashMap::new();

        for (server_name, server_config) in config.mcp_servers {
            match Self::connect_server(&server_name, &server_config).await {
                Ok((client, discovered_tools)) => {
                    for tool in discovered_tools {
                        let original_tool_name = tool.name.trim().to_string();
                        if original_tool_name.is_empty() {
                            continue;
                        }

                        let base_name = exposed_tool_name(&server_name, &original_tool_name);
                        let exposed_name = unique_tool_name(&base_name, &tool_index);
                        let description = tool
                            .description
                            .as_deref()
                            .map(str::to_string)
                            .unwrap_or_else(|| format!("MCP tool from {server_name} server"));

                        tools.push(DiscoveredMcpTool {
                            exposed_name: exposed_name.clone(),
                            description: format!("[MCP:{server_name}] {description}"),
                            parameters: tool.schema_as_json_value(),
                        });
                        tool_index.insert(
                            exposed_name,
                            McpToolRef {
                                server_name: server_name.clone(),
                                original_tool_name,
                            },
                        );
                    }

                    clients.insert(server_name, client);
                }
                Err(err) => {
                    log::warn!("failed to connect MCP server {server_name}: {err}");
                }
            }
        }

        Ok(Self {
            clients,
            tools,
            tool_index,
        })
    }

    async fn connect_server(
        server_name: &str,
        config: &McpServerConfig,
    ) -> Result<(McpClient, Vec<RmcpTool>), ToolError> {
        let command = config.command.as_deref().ok_or_else(|| {
            ToolError::ExecutionFailed(format!("MCP server {server_name} requires command"))
        })?;
        if command.trim().is_empty() {
            return Err(ToolError::ExecutionFailed(format!(
                "MCP server {server_name} command cannot be empty"
            )));
        }

        let transport = TokioChildProcess::new(Command::new(command).configure(|cmd| {
            cmd.args(&config.args).envs(&config.env);
        }))
        .map_err(|err| ToolError::ExecutionFailed(err.to_string()))?;

        let client = ().serve(transport).await.map_err(to_tool_error)?;
        let tools = client.list_all_tools().await.map_err(to_tool_error)?;

        Ok((client, tools))
    }

    pub fn tools(&self) -> Vec<DiscoveredMcpTool> {
        self.tools.clone()
    }

    pub async fn call_tool(
        &self,
        exposed_name: &str,
        arguments: Value,
    ) -> Result<Value, ToolError> {
        let tool_ref = self.tool_index.get(exposed_name).ok_or_else(|| {
            ToolError::ExecutionFailed(format!("unknown MCP tool: {exposed_name}"))
        })?;

        let client = self.clients.get(&tool_ref.server_name).ok_or_else(|| {
            ToolError::ExecutionFailed(format!(
                "MCP server not connected: {}",
                tool_ref.server_name
            ))
        })?;

        let request = CallToolRequestParams::new(tool_ref.original_tool_name.clone())
            .with_arguments(arguments_object(arguments)?);
        let result = client.call_tool(request).await.map_err(to_tool_error)?;

        serde_json::to_value(result).map_err(|err| ToolError::ExecutionFailed(err.to_string()))
    }
}

fn arguments_object(arguments: Value) -> Result<JsonObject, ToolError> {
    match arguments {
        Value::Object(map) => Ok(map),
        Value::Null => Ok(JsonObject::new()),
        other => Err(ToolError::ExecutionFailed(format!(
            "MCP tool arguments must be a JSON object, got {other}"
        ))),
    }
}

fn to_tool_error(err: impl std::fmt::Display) -> ToolError {
    ToolError::ExecutionFailed(err.to_string())
}

fn unique_tool_name(base_name: &str, tool_index: &HashMap<String, McpToolRef>) -> String {
    if !tool_index.contains_key(base_name) {
        return base_name.to_string();
    }

    let mut suffix = 2;
    loop {
        let candidate = format!("{base_name}__{suffix}");
        if !tool_index.contains_key(&candidate) {
            return candidate;
        }
        suffix += 1;
    }
}

fn exposed_tool_name(server: &str, tool: &str) -> String {
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
