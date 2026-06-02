use serde::Deserialize;
use std::{collections::HashMap, path::Path};
use tokio::fs;

#[derive(Debug, Clone, Deserialize, Default)]
pub struct McpServerConfig {
    pub command: Option<String>,

    #[serde(default)]
    pub args: Vec<String>,

    #[serde(default)]
    pub env: HashMap<String, String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct McpConfig {
    #[serde(rename = "mcpServers", default)]
    pub mcp_servers: HashMap<String, McpServerConfig>,
}

impl McpConfig {
    pub async fn load_optional(path: &Path) -> Result<Option<Self>, std::io::Error> {
        if !fs::try_exists(path).await? {
            return Ok(None);
        }

        let content = fs::read_to_string(path).await?;
        let config = serde_json::from_str(&content)
            .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidData, err))?;

        Ok(Some(config))
    }
}
