use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::application::error::schedule_usecase_error::ScheduleUsecaseError;
use crate::domain::error::schedule_repository_error::ScheduleRepositoryError;
use crate::domain::model::schedule::Schedule;
use crate::domain::model::schedule_execution::ScheduleExecution;
use crate::domain::model::task::{Task, TaskSourceKind};
use crate::domain::repository::schedule_repository::{
    CreateSchedule, ScheduleRepository, UpdateSchedule,
};
use crate::domain::repository::task_repository::{CreateTask, TaskRepository};

pub struct ScheduledTaskStart {
    pub execution: ScheduleExecution,
    pub task: Task,
}

pub enum ScheduleExecutionOutcome {
    Started(ScheduledTaskStart),
    AlreadyRecorded(ScheduleExecution),
}

pub struct ScheduleUsecase<S, TaskRepo> {
    schedule_repository: S,
    task_repository: TaskRepo,
}

impl<S, TaskRepo> ScheduleUsecase<S, TaskRepo>
where
    S: ScheduleRepository,
    TaskRepo: TaskRepository,
{
    pub fn new(schedule_repository: S, task_repository: TaskRepo) -> Self {
        Self {
            schedule_repository,
            task_repository,
        }
    }

    pub async fn create(
        &self,
        title: String,
        request: String,
        cron: String,
        timezone: String,
        enabled: bool,
    ) -> Result<Schedule, ScheduleUsecaseError> {
        self.schedule_repository
            .create(CreateSchedule {
                title,
                request,
                cron,
                timezone,
                enabled,
            })
            .await
            .map_err(Into::into)
    }

    pub async fn list(&self) -> Result<Vec<Schedule>, ScheduleUsecaseError> {
        self.schedule_repository.list().await.map_err(Into::into)
    }

    pub async fn list_enabled(&self) -> Result<Vec<Schedule>, ScheduleUsecaseError> {
        self.schedule_repository
            .list_enabled()
            .await
            .map_err(Into::into)
    }

    pub async fn find(&self, id: Uuid) -> Result<Option<Schedule>, ScheduleUsecaseError> {
        self.schedule_repository
            .find_by_id(id)
            .await
            .map_err(Into::into)
    }

    pub async fn update(
        &self,
        id: Uuid,
        input: UpdateSchedule,
    ) -> Result<Schedule, ScheduleUsecaseError> {
        self.schedule_repository
            .update(id, input)
            .await
            .map_err(Into::into)
    }

    pub async fn run_now(
        &self,
        schedule_id: Uuid,
    ) -> Result<ScheduledTaskStart, ScheduleUsecaseError> {
        self.run_at(schedule_id, Utc::now()).await
    }

    pub async fn run_at(
        &self,
        schedule_id: Uuid,
        scheduled_at: DateTime<Utc>,
    ) -> Result<ScheduledTaskStart, ScheduleUsecaseError> {
        let schedule = self
            .schedule_repository
            .find_by_id(schedule_id)
            .await?
            .ok_or(ScheduleRepositoryError::NotFound(schedule_id))?;

        let task = self
            .task_repository
            .create(CreateTask {
                request: schedule.request,
                session_id: None,
                source_kind: TaskSourceKind::Schedule,
                source_message_id: None,
                source_schedule_id: Some(schedule_id),
                source_tool_call_id: None,
                subagent_profile: None,
                parent_task_id: None,
                scheduled_at: Some(scheduled_at),
            })
            .await?;

        let execution = execution_from_task(schedule_id, &task);

        Ok(ScheduledTaskStart { execution, task })
    }

    pub async fn list_executions(
        &self,
        schedule_id: Uuid,
    ) -> Result<Vec<ScheduleExecution>, ScheduleUsecaseError> {
        if self
            .schedule_repository
            .find_by_id(schedule_id)
            .await?
            .is_none()
        {
            return Err(ScheduleRepositoryError::NotFound(schedule_id).into());
        }

        let tasks = self
            .task_repository
            .list_by_source_schedule_id(schedule_id)
            .await?;

        Ok(tasks
            .into_iter()
            .map(|task| execution_from_task(schedule_id, &task))
            .collect())
    }

    pub async fn run_once_at(
        &self,
        schedule_id: Uuid,
        scheduled_at: DateTime<Utc>,
    ) -> Result<ScheduleExecutionOutcome, ScheduleUsecaseError> {
        if let Some(task) = self
            .task_repository
            .find_by_source_schedule_id_and_scheduled_at(schedule_id, scheduled_at)
            .await?
        {
            return Ok(ScheduleExecutionOutcome::AlreadyRecorded(
                execution_from_task(schedule_id, &task),
            ));
        }

        self.run_at(schedule_id, scheduled_at)
            .await
            .map(ScheduleExecutionOutcome::Started)
    }
}

fn execution_from_task(schedule_id: Uuid, task: &Task) -> ScheduleExecution {
    ScheduleExecution {
        id: task.id,
        schedule_id,
        task_id: task.id,
        scheduled_at: task.scheduled_at.unwrap_or(task.created_at),
        created_at: task.created_at,
    }
}
