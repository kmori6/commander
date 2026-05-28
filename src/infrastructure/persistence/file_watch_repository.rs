use async_trait::async_trait;
use serde::Deserialize;
use std::path::PathBuf;

use crate::domain::error::watch_repository_error::WatchRepositoryError;
use crate::domain::model::watch::{Watch, WatchSchedule};
use crate::domain::repository::watch_repository::WatchRepository;

#[derive(Clone)]
pub struct FileWatchRepository {
    path: PathBuf,
}

#[derive(Debug, Deserialize)]
struct StoredWatchConfig {
    #[serde(default)]
    enabled: bool,
    #[serde(default)]
    schedules: Vec<StoredWatchSchedule>,
}

#[derive(Debug, Deserialize)]
struct StoredWatchSchedule {
    cron: String,
    timezone: String,
}

impl FileWatchRepository {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }
}

#[async_trait]
impl WatchRepository for FileWatchRepository {
    async fn get(&self) -> Result<Option<Watch>, WatchRepositoryError> {
        let content = match tokio::fs::read_to_string(&self.path).await {
            Ok(content) => content,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(err) => return Err(WatchRepositoryError::Unexpected(err.to_string())),
        };

        let stored = serde_json::from_str::<StoredWatchConfig>(&content)
            .map_err(|err| WatchRepositoryError::InvalidConfig(err.to_string()))?;

        let schedules = stored
            .schedules
            .into_iter()
            .map(|schedule| WatchSchedule::try_new(schedule.cron, schedule.timezone))
            .collect::<Result<Vec<_>, _>>()
            .map_err(WatchRepositoryError::InvalidConfig)?;

        Ok(Some(Watch {
            enabled: stored.enabled,
            schedules,
        }))
    }
}
