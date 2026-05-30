use serde::Serialize;

use crate::domain::model::task::Task;

#[derive(Debug, Serialize)]
pub struct TaskResponse {
    pub id: String,
    pub status: String,
    pub session_id: Option<String>,
    pub schedule_id: Option<String>,
    pub scheduled_at: Option<String>,
    pub error: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
}

impl From<Task> for TaskResponse {
    fn from(task: Task) -> Self {
        Self {
            id: task.id.to_string(),
            status: task.status.as_str().to_string(),
            session_id: task.session_id().map(|id| id.to_string()),
            schedule_id: task.schedule_id().map(|id| id.to_string()),
            scheduled_at: task.scheduled_at().map(|dt| dt.to_rfc3339()),
            error: task.error,
            created_at: task.created_at.to_rfc3339(),
            updated_at: task.updated_at.to_rfc3339(),
            started_at: task.started_at.map(|dt| dt.to_rfc3339()),
            finished_at: task.finished_at.map(|dt| dt.to_rfc3339()),
        }
    }
}
