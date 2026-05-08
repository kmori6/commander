use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Schedule {
    pub id: Uuid,
    pub title: String,
    pub request: String,
    pub cron: String,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScheduleRun {
    pub id: Uuid,
    pub schedule_id: Uuid,
    pub task_id: Uuid,
    pub scheduled_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}
