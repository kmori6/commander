use async_trait::async_trait;
use serde::Deserialize;
use std::path::PathBuf;

use crate::domain::error::subagent_repository_error::SubagentRepositoryError;
use crate::domain::model::subagent::Subagent;
use crate::domain::repository::subagent_repository::SubagentRepository;

#[derive(Clone)]
pub struct FileSubagentRepository {
    root: PathBuf,
}

#[derive(Debug, Deserialize)]
struct StoredSubagent {
    #[serde(default)]
    description: String,
    instruction: String,
    #[serde(default)]
    allowed_tools: Vec<String>,
}

impl FileSubagentRepository {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }
}

#[async_trait]
impl SubagentRepository for FileSubagentRepository {
    async fn list(&self) -> Result<Vec<Subagent>, SubagentRepositoryError> {
        let mut entries = match tokio::fs::read_dir(&self.root).await {
            Ok(entries) => entries,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(err) => return Err(SubagentRepositoryError::Unexpected(err.to_string())),
        };

        let mut subagents = Vec::new();

        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|err| SubagentRepositoryError::Unexpected(err.to_string()))?
        {
            let path = entry.path();

            if path.extension().and_then(|value| value.to_str()) != Some("json") {
                continue;
            }

            let Some(name) = path.file_stem().and_then(|value| value.to_str()) else {
                continue;
            };

            let content = tokio::fs::read_to_string(&path)
                .await
                .map_err(|err| SubagentRepositoryError::Unexpected(err.to_string()))?;

            let stored = match serde_json::from_str::<StoredSubagent>(&content) {
                Ok(stored) => stored,
                Err(err) => {
                    log::warn!("invalid subagent profile {}: {err}", path.display());
                    continue;
                }
            };

            match Subagent::restore(
                name,
                stored.description,
                stored.instruction,
                stored.allowed_tools,
            ) {
                Ok(subagent) => subagents.push(subagent),
                Err(err) => log::warn!("invalid subagent profile {}: {err}", path.display()),
            }
        }

        subagents.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(subagents)
    }
}
