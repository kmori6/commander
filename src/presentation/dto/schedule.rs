use serde::Serialize;
use uuid::Uuid;

use crate::domain::model::schedule::Schedule;
use crate::domain::model::task::Task;

#[derive(Debug, Serialize)]
pub struct ScheduleResponse {
    pub id: String,
    pub title: String,
    pub request: String,
    pub cron: String,
    pub timezone: String,
    pub enabled: bool,
    pub created_at: String,
    pub updated_at: String,
}

impl From<Schedule> for ScheduleResponse {
    fn from(schedule: Schedule) -> Self {
        Self {
            id: schedule.id.to_string(),
            title: schedule.title,
            request: schedule.request,
            cron: schedule.cron.as_str().to_string(),
            timezone: schedule.timezone.as_str().to_string(),
            enabled: schedule.enabled,
            created_at: schedule.created_at.to_rfc3339(),
            updated_at: schedule.updated_at.to_rfc3339(),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct ScheduleRunResponse {
    pub id: String,
    pub schedule_id: String,
    pub task_id: String,
    pub scheduled_at: String,
    pub created_at: String,
}

impl ScheduleRunResponse {
    pub fn new(schedule_id: Uuid, task: &Task) -> Self {
        Self {
            id: task.id.to_string(),
            schedule_id: schedule_id.to_string(),
            task_id: task.id.to_string(),
            scheduled_at: task.scheduled_at().unwrap_or(task.created_at).to_rfc3339(),
            created_at: task.created_at.to_rfc3339(),
        }
    }
}
