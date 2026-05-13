use chrono::{DateTime, Utc};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScheduleExecution {
    pub id: Uuid,
    pub schedule_id: Uuid,
    pub task_id: Uuid,
    pub scheduled_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}
