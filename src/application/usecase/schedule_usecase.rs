use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::application::error::schedule_usecase_error::ScheduleUsecaseError;
use crate::domain::error::schedule_repository_error::ScheduleRepositoryError;
use crate::domain::model::schedule::Schedule;
use crate::domain::model::schedule_execution::ScheduleExecution;
use crate::domain::model::session::SessionKind;
use crate::domain::model::task::Task;
use crate::domain::repository::schedule_repository::{
    CreateSchedule, ScheduleRepository, UpdateSchedule,
};
use crate::domain::repository::session_repository::SessionRepository;
use crate::domain::repository::task_repository::{CreateTask, TaskRepository};

pub struct ScheduledTaskStart {
    pub execution: ScheduleExecution,
    pub task: Task,
}

pub enum ScheduleExecutionOutcome {
    Started(ScheduledTaskStart),
    AlreadyRecorded(ScheduleExecution),
}

pub struct ScheduleUsecase<S, SessionRepo, TaskRepo> {
    schedule_repository: S,
    session_repository: SessionRepo,
    task_repository: TaskRepo,
}

impl<S, SessionRepo, TaskRepo> ScheduleUsecase<S, SessionRepo, TaskRepo>
where
    S: ScheduleRepository,
    SessionRepo: SessionRepository,
    TaskRepo: TaskRepository,
{
    pub fn new(
        schedule_repository: S,
        session_repository: SessionRepo,
        task_repository: TaskRepo,
    ) -> Self {
        Self {
            schedule_repository,
            session_repository,
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

        let task_session = self
            .session_repository
            .create(SessionKind::Task, Some(schedule.title.clone()))
            .await?;

        let task = self
            .task_repository
            .create(CreateTask {
                request: schedule.request,
                session_id: task_session.id,
                source_message_id: None,
                parent_task_id: None,
            })
            .await?;

        let execution = self
            .schedule_repository
            .record_execution(schedule_id, task.id, scheduled_at)
            .await?;

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

        self.schedule_repository
            .list_executions(schedule_id)
            .await
            .map_err(Into::into)
    }

    pub async fn run_once_at(
        &self,
        schedule_id: Uuid,
        scheduled_at: DateTime<Utc>,
    ) -> Result<ScheduleExecutionOutcome, ScheduleUsecaseError> {
        if let Some(execution) = self
            .schedule_repository
            .find_execution_by_schedule_and_scheduled_at(schedule_id, scheduled_at)
            .await?
        {
            return Ok(ScheduleExecutionOutcome::AlreadyRecorded(execution));
        }

        self.run_at(schedule_id, scheduled_at)
            .await
            .map(ScheduleExecutionOutcome::Started)
    }
}
