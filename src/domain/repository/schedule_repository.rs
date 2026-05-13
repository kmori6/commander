use async_trait::async_trait;
use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::domain::error::schedule_repository_error::ScheduleRepositoryError;
use crate::domain::model::schedule::Schedule;
use crate::domain::model::schedule_execution::ScheduleExecution;

#[derive(Debug, Clone)]
pub struct CreateSchedule {
    pub title: String,
    pub request: String,
    pub cron: String,
    pub timezone: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, Default)]
pub struct UpdateSchedule {
    pub title: Option<String>,
    pub request: Option<String>,
    pub cron: Option<String>,
    pub timezone: Option<String>,
    pub enabled: Option<bool>,
}

#[async_trait]
pub trait ScheduleRepository: Send + Sync {
    async fn create(&self, input: CreateSchedule) -> Result<Schedule, ScheduleRepositoryError>;

    async fn find_by_id(&self, id: Uuid) -> Result<Option<Schedule>, ScheduleRepositoryError>;

    async fn list(&self) -> Result<Vec<Schedule>, ScheduleRepositoryError>;

    async fn update(
        &self,
        id: Uuid,
        input: UpdateSchedule,
    ) -> Result<Schedule, ScheduleRepositoryError>;

    async fn record_execution(
        &self,
        schedule_id: Uuid,
        task_id: Uuid,
        scheduled_at: DateTime<Utc>,
    ) -> Result<ScheduleExecution, ScheduleRepositoryError>;

    async fn list_executions(
        &self,
        schedule_id: Uuid,
    ) -> Result<Vec<ScheduleExecution>, ScheduleRepositoryError>;

    async fn list_enabled(&self) -> Result<Vec<Schedule>, ScheduleRepositoryError>;

    async fn find_execution_by_schedule_and_scheduled_at(
        &self,
        schedule_id: Uuid,
        scheduled_at: DateTime<Utc>,
    ) -> Result<Option<ScheduleExecution>, ScheduleRepositoryError>;
}
