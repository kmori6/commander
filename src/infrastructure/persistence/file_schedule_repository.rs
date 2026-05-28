use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::domain::error::schedule_repository_error::ScheduleRepositoryError;
use crate::domain::model::schedule::{CronExpression, Schedule, ScheduleTimezone};
use crate::domain::repository::schedule_repository::{
    CreateSchedule, ScheduleRepository, UpdateSchedule,
};

const VERSION: u32 = 1;

#[derive(Clone)]
pub struct FileScheduleRepository {
    path: PathBuf,
    lock: Arc<Mutex<()>>,
}

#[derive(Debug, Serialize, Deserialize)]
struct SchedulesFile {
    version: u32,
    schedules: Vec<StoredSchedule>,
}

impl Default for SchedulesFile {
    fn default() -> Self {
        Self {
            version: VERSION,
            schedules: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredSchedule {
    id: Uuid,
    title: String,
    request: String,
    cron: String,
    timezone: String,
    enabled: bool,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl StoredSchedule {
    fn try_into_model(self) -> Result<Schedule, ScheduleRepositoryError> {
        let id = self.id;
        stored_schedule_from(self).map_err(|message| {
            ScheduleRepositoryError::Unexpected(format!("invalid stored schedule {id}: {message}"))
        })
    }
}

fn stored_schedule_from(stored: StoredSchedule) -> Result<Schedule, String> {
    Ok(Schedule {
        id: stored.id,
        title: required_text("title", stored.title)?,
        request: required_text("request", stored.request)?,
        cron: CronExpression::parse(&stored.cron)?,
        timezone: ScheduleTimezone::parse(&stored.timezone)?,
        enabled: stored.enabled,
        created_at: stored.created_at,
        updated_at: stored.updated_at,
    })
}

fn required_text(field: &str, value: String) -> Result<String, String> {
    let value = value.trim().to_string();

    if value.is_empty() {
        return Err(format!("{field} must not be empty"));
    }

    Ok(value)
}

impl FileScheduleRepository {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            lock: Arc::new(Mutex::new(())),
        }
    }

    async fn load(&self) -> Result<SchedulesFile, ScheduleRepositoryError> {
        let content = match tokio::fs::read_to_string(&self.path).await {
            Ok(content) => content,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                return Ok(SchedulesFile::default());
            }
            Err(err) => return Err(map_io_error(err)),
        };

        let file = serde_json::from_str::<SchedulesFile>(&content)
            .map_err(|err| ScheduleRepositoryError::Unexpected(err.to_string()))?;

        if file.version != VERSION {
            return Err(ScheduleRepositoryError::Unexpected(format!(
                "unsupported schedules file version: {}",
                file.version
            )));
        }

        Ok(file)
    }

    async fn save(&self, file: &SchedulesFile) -> Result<(), ScheduleRepositoryError> {
        let parent = self.path.parent().ok_or_else(|| {
            ScheduleRepositoryError::Unexpected(
                "schedules path must include a parent directory".to_string(),
            )
        })?;

        tokio::fs::create_dir_all(parent)
            .await
            .map_err(map_io_error)?;

        let mut content = serde_json::to_string_pretty(file)
            .map_err(|err| ScheduleRepositoryError::Unexpected(err.to_string()))?;
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
impl ScheduleRepository for FileScheduleRepository {
    async fn create(&self, input: CreateSchedule) -> Result<Schedule, ScheduleRepositoryError> {
        let _guard = self.lock.lock().await;

        let mut file = self.load().await?;
        let now = Utc::now();
        let schedule = stored_schedule_from(StoredSchedule {
            id: Uuid::new_v4(),
            title: input.title,
            request: input.request,
            cron: input.cron,
            timezone: input.timezone,
            enabled: input.enabled,
            created_at: now,
            updated_at: now,
        })
        .map_err(ScheduleRepositoryError::InvalidSchedule)?;

        file.schedules.push(StoredSchedule {
            id: schedule.id,
            title: schedule.title.clone(),
            request: schedule.request.clone(),
            cron: schedule.cron.as_str().to_string(),
            timezone: schedule.timezone.as_str().to_string(),
            enabled: schedule.enabled,
            created_at: schedule.created_at,
            updated_at: schedule.updated_at,
        });
        sort_schedules(&mut file.schedules);
        self.save(&file).await?;

        Ok(schedule)
    }

    async fn find_by_id(&self, id: Uuid) -> Result<Option<Schedule>, ScheduleRepositoryError> {
        let file = self.load().await?;

        file.schedules
            .into_iter()
            .find(|schedule| schedule.id == id)
            .map(StoredSchedule::try_into_model)
            .transpose()
    }

    async fn list(&self) -> Result<Vec<Schedule>, ScheduleRepositoryError> {
        let mut file = self.load().await?;
        sort_schedules(&mut file.schedules);

        file.schedules
            .into_iter()
            .map(StoredSchedule::try_into_model)
            .collect()
    }

    async fn update(
        &self,
        id: Uuid,
        input: UpdateSchedule,
    ) -> Result<Schedule, ScheduleRepositoryError> {
        let _guard = self.lock.lock().await;

        let mut file = self.load().await?;
        let schedule = file
            .schedules
            .iter_mut()
            .find(|schedule| schedule.id == id)
            .ok_or(ScheduleRepositoryError::NotFound(id))?;

        let updated = stored_schedule_from(StoredSchedule {
            id: schedule.id,
            title: input.title.unwrap_or_else(|| schedule.title.clone()),
            request: input.request.unwrap_or_else(|| schedule.request.clone()),
            cron: input.cron.unwrap_or_else(|| schedule.cron.clone()),
            timezone: input.timezone.unwrap_or_else(|| schedule.timezone.clone()),
            enabled: input.enabled.unwrap_or(schedule.enabled),
            created_at: schedule.created_at,
            updated_at: Utc::now(),
        })
        .map_err(ScheduleRepositoryError::InvalidSchedule)?;

        schedule.title = updated.title.clone();
        schedule.request = updated.request.clone();
        schedule.cron = updated.cron.as_str().to_string();
        schedule.timezone = updated.timezone.as_str().to_string();
        schedule.enabled = updated.enabled;
        schedule.updated_at = updated.updated_at;

        sort_schedules(&mut file.schedules);
        self.save(&file).await?;

        Ok(updated)
    }
}

fn sort_schedules(schedules: &mut [StoredSchedule]) {
    schedules.sort_by(|left, right| {
        right
            .updated_at
            .cmp(&left.updated_at)
            .then_with(|| right.id.cmp(&left.id))
    });
}

fn map_io_error(err: std::io::Error) -> ScheduleRepositoryError {
    ScheduleRepositoryError::Unexpected(err.to_string())
}
