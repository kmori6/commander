use serde::Deserialize;
use std::{collections::HashMap, path::Path};
use tokio::fs;

// NOTE: currently supports MCP servers launched over stdio.
#[derive(Debug, Deserialize)]
pub struct McpServerConfig {
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
}

#[derive(Debug, Deserialize)]
pub struct McpConfig {
    #[serde(rename = "mcpServers", default)]
    pub servers: HashMap<String, McpServerConfig>,
}

impl McpConfig {
    pub async fn load_optional(path: &Path) -> std::io::Result<Option<Self>> {
        if !fs::try_exists(path).await? {
            return Ok(None);
        }

        let content = fs::read_to_string(path).await?;
        serde_json::from_str(&content)
            .map(Some)
            .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidData, err))
    }
}
