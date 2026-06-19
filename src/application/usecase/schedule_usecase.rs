use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::application::error::schedule_usecase_error::ScheduleUsecaseError;
use crate::domain::error::schedule_repository_error::ScheduleRepositoryError;
use crate::domain::model::schedule::Schedule;
use crate::domain::model::task::{Task, TaskSource};
use crate::domain::repository::schedule_repository::{
    CreateSchedule, ScheduleRepository, UpdateSchedule,
};
use crate::domain::repository::task_repository::TaskRepository;

pub enum DueTaskOutcome {
    Started(Task),
    AlreadyRecorded(Task),
    NoRequest,
}

pub enum ScheduleRunOutcome {
    Started(Task),
    AlreadyRecorded(Task),
}

pub struct ScheduleUsecase<S, T> {
    schedule_repository: S,
    task_repository: T,
}

impl<S, T> ScheduleUsecase<S, T>
where
    S: ScheduleRepository,
    T: TaskRepository,
{
    pub fn new(schedule_repository: S, task_repository: T) -> Self {
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

    pub async fn run_now(&self, schedule_id: Uuid) -> Result<Task, ScheduleUsecaseError> {
        self.run_at(schedule_id, Utc::now()).await
    }

    pub async fn run_at(
        &self,
        schedule_id: Uuid,
        scheduled_at: DateTime<Utc>,
    ) -> Result<Task, ScheduleUsecaseError> {
        let schedule = self
            .schedule_repository
            .find_by_id(schedule_id)
            .await?
            .ok_or(ScheduleRepositoryError::NotFound(schedule_id))?;

        match self
            .run_due_task(schedule.request, Some(schedule_id), scheduled_at)
            .await?
        {
            DueTaskOutcome::Started(task) => Ok(task),
            DueTaskOutcome::AlreadyRecorded(_) => Err(ScheduleRepositoryError::InvalidSchedule(
                "schedule run already recorded".to_string(),
            )
            .into()),
            DueTaskOutcome::NoRequest => Err(ScheduleRepositoryError::InvalidSchedule(
                "schedule did not start".to_string(),
            )
            .into()),
        }
    }

    pub async fn list_runs(&self, schedule_id: Uuid) -> Result<Vec<Task>, ScheduleUsecaseError> {
        if self
            .schedule_repository
            .find_by_id(schedule_id)
            .await?
            .is_none()
        {
            return Err(ScheduleRepositoryError::NotFound(schedule_id).into());
        }

        self.task_repository
            .list_runs(schedule_id)
            .await
            .map_err(Into::into)
    }

    pub async fn run_once_at(
        &self,
        schedule_id: Uuid,
        scheduled_at: DateTime<Utc>,
    ) -> Result<ScheduleRunOutcome, ScheduleUsecaseError> {
        let schedule = self
            .schedule_repository
            .find_by_id(schedule_id)
            .await?
            .ok_or(ScheduleRepositoryError::NotFound(schedule_id))?;

        match self
            .run_due_task(schedule.request, Some(schedule_id), scheduled_at)
            .await?
        {
            DueTaskOutcome::Started(task) => Ok(ScheduleRunOutcome::Started(task)),
            DueTaskOutcome::AlreadyRecorded(task) => Ok(ScheduleRunOutcome::AlreadyRecorded(task)),
            DueTaskOutcome::NoRequest => Err(ScheduleRepositoryError::InvalidSchedule(
                "schedule did not start".to_string(),
            )
            .into()),
        }
    }

    pub async fn run_due_task(
        &self,
        request: String,
        schedule_id: Option<Uuid>,
        scheduled_at: DateTime<Utc>,
    ) -> Result<DueTaskOutcome, ScheduleUsecaseError> {
        let request = request.trim().to_string();

        if request.is_empty() {
            return Ok(DueTaskOutcome::NoRequest);
        }

        if let Some(schedule_id) = schedule_id
            && let Some(task) = self
                .task_repository
                .find_run(schedule_id, scheduled_at)
                .await?
        {
            return Ok(DueTaskOutcome::AlreadyRecorded(task));
        }

        let source = match schedule_id {
            Some(schedule_id) => TaskSource::Schedule {
                schedule_id,
                scheduled_at,
            },
            None => TaskSource::Watch { scheduled_at },
        };

        let task = self.task_repository.enqueue(source, request).await?;

        Ok(DueTaskOutcome::Started(task))
    }
}
