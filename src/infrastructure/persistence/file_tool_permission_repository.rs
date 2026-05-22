use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::domain::error::tool_permission_repository_error::ToolPermissionRepositoryError;
use crate::domain::model::tool_call::{ToolPermission, ToolPermissionMode};
use crate::domain::repository::tool_permission_repository::ToolPermissionRepository;

const VERSION: u32 = 1;

#[derive(Clone)]
pub struct FileToolPermissionRepository {
    path: PathBuf,
    lock: Arc<Mutex<()>>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ToolPermissionsFile {
    version: u32,
    permissions: BTreeMap<String, ToolPermissionMode>,
}

impl Default for ToolPermissionsFile {
    fn default() -> Self {
        Self {
            version: VERSION,
            permissions: BTreeMap::new(),
        }
    }
}

impl FileToolPermissionRepository {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            lock: Arc::new(Mutex::new(())),
        }
    }

    async fn load(&self) -> Result<ToolPermissionsFile, ToolPermissionRepositoryError> {
        let content = match tokio::fs::read_to_string(&self.path).await {
            Ok(content) => content,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                return Ok(ToolPermissionsFile::default());
            }
            Err(err) => return Err(map_io_error(err)),
        };

        let file: ToolPermissionsFile = serde_json::from_str(&content)
            .map_err(|err| ToolPermissionRepositoryError::Unexpected(err.to_string()))?;

        if file.version != VERSION {
            return Err(ToolPermissionRepositoryError::Unexpected(format!(
                "unsupported tool permissions file version: {}",
                file.version
            )));
        }

        Ok(file)
    }

    async fn save(&self, file: &ToolPermissionsFile) -> Result<(), ToolPermissionRepositoryError> {
        let parent = self.path.parent().ok_or_else(|| {
            ToolPermissionRepositoryError::Unexpected(
                "tool permissions path must include a parent directory".to_string(),
            )
        })?;

        tokio::fs::create_dir_all(parent)
            .await
            .map_err(map_io_error)?;

        let mut content = serde_json::to_string_pretty(file)
            .map_err(|err| ToolPermissionRepositoryError::Unexpected(err.to_string()))?;
        content.push('\n');

        let tmp_path = self.path.with_extension("json.tmp");

        tokio::fs::write(&tmp_path, content)
            .await
            .map_err(map_io_error)?;

        tokio::fs::rename(&tmp_path, &self.path)
            .await
            .map_err(map_io_error)?;

        Ok(())
    }
}

#[async_trait]
impl ToolPermissionRepository for FileToolPermissionRepository {
    async fn list(&self) -> Result<Vec<ToolPermission>, ToolPermissionRepositoryError> {
        let file = self.load().await?;

        Ok(file
            .permissions
            .into_iter()
            .map(|(tool_name, mode)| ToolPermission { tool_name, mode })
            .collect())
    }

    async fn upsert(
        &self,
        tool_name: &str,
        mode: ToolPermissionMode,
    ) -> Result<ToolPermission, ToolPermissionRepositoryError> {
        let tool_name = normalize_tool_name(tool_name)?;

        let _guard = self.lock.lock().await;

        let mut file = self.load().await?;
        file.permissions.insert(tool_name.clone(), mode);
        self.save(&file).await?;

        Ok(ToolPermission { tool_name, mode })
    }

    async fn find(
        &self,
        tool_name: &str,
    ) -> Result<Option<ToolPermission>, ToolPermissionRepositoryError> {
        let tool_name = tool_name.trim();
        if tool_name.is_empty() {
            return Ok(None);
        }

        let file = self.load().await?;

        Ok(file
            .permissions
            .get(tool_name)
            .copied()
            .map(|mode| ToolPermission {
                tool_name: tool_name.to_string(),
                mode,
            }))
    }
}

fn normalize_tool_name(tool_name: &str) -> Result<String, ToolPermissionRepositoryError> {
    let tool_name = tool_name.trim();

    if tool_name.is_empty() {
        return Err(ToolPermissionRepositoryError::InvalidPermission(
            "tool_name must not be empty".to_string(),
        ));
    }

    Ok(tool_name.to_string())
}

fn map_io_error(err: std::io::Error) -> ToolPermissionRepositoryError {
    ToolPermissionRepositoryError::Unexpected(err.to_string())
}
