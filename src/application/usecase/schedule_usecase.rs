use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::application::error::schedule_usecase_error::ScheduleUsecaseError;
use crate::domain::error::schedule_repository_error::ScheduleRepositoryError;
use crate::domain::model::message::{MessageContent, Role};
use crate::domain::model::schedule::Schedule;
use crate::domain::model::task::Task;
use crate::domain::repository::message_repository::MessageRepository;
use crate::domain::repository::schedule_repository::{
    CreateSchedule, ScheduleRepository, UpdateSchedule,
};
use crate::domain::repository::task_repository::TaskRepository;

pub struct DueTaskInput {
    pub request: String,
    pub schedule_id: Option<Uuid>,
    pub scheduled_at: DateTime<Utc>,
    pub skip_if_open_same_source: bool,
}

pub enum DueTaskOutcome {
    Started(Task),
    AlreadyRecorded(Task),
    AlreadyRunning(Task),
    NoRequest,
}

pub enum ScheduleRunOutcome {
    Started(Task),
    AlreadyRecorded(Task),
}

pub struct ScheduleUsecase<S, T, M> {
    schedule_repository: S,
    task_repository: T,
    message_repository: M,
}

impl<S, T, M> ScheduleUsecase<S, T, M>
where
    S: ScheduleRepository,
    T: TaskRepository,
    M: MessageRepository,
{
    pub fn new(schedule_repository: S, task_repository: T, message_repository: M) -> Self {
        Self {
            schedule_repository,
            task_repository,
            message_repository,
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
            .run_due_task(DueTaskInput {
                request: schedule.request,
                schedule_id: Some(schedule_id),
                scheduled_at,
                skip_if_open_same_source: false,
            })
            .await?
        {
            DueTaskOutcome::Started(task) => Ok(task),
            DueTaskOutcome::AlreadyRecorded(_) => Err(ScheduleRepositoryError::InvalidSchedule(
                "schedule run already recorded".to_string(),
            )
            .into()),
            DueTaskOutcome::AlreadyRunning(_) | DueTaskOutcome::NoRequest => Err(
                ScheduleRepositoryError::InvalidSchedule("schedule did not start".to_string())
                    .into(),
            ),
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
            .list_by_schedule_id(schedule_id)
            .await
            .map_err(Into::into)
    }

    pub async fn run_once_at(
        &self,
        schedule_id: Uuid,
        scheduled_at: DateTime<Utc>,
    ) -> Result<ScheduleRunOutcome, ScheduleUsecaseError> {
        if let Some(task) = self
            .task_repository
            .find_by_schedule_id_and_scheduled_at(schedule_id, scheduled_at)
            .await?
        {
            return Ok(ScheduleRunOutcome::AlreadyRecorded(task));
        }

        let schedule = self
            .schedule_repository
            .find_by_id(schedule_id)
            .await?
            .ok_or(ScheduleRepositoryError::NotFound(schedule_id))?;

        match self
            .run_due_task(DueTaskInput {
                request: schedule.request,
                schedule_id: Some(schedule_id),
                scheduled_at,
                skip_if_open_same_source: false,
            })
            .await?
        {
            DueTaskOutcome::Started(task) => Ok(ScheduleRunOutcome::Started(task)),
            DueTaskOutcome::AlreadyRecorded(task) => Ok(ScheduleRunOutcome::AlreadyRecorded(task)),
            DueTaskOutcome::AlreadyRunning(_) | DueTaskOutcome::NoRequest => Err(
                ScheduleRepositoryError::InvalidSchedule("schedule did not start".to_string())
                    .into(),
            ),
        }
    }

    pub async fn run_due_task(
        &self,
        input: DueTaskInput,
    ) -> Result<DueTaskOutcome, ScheduleUsecaseError> {
        let request = input.request.trim().to_string();

        if request.is_empty() {
            return Ok(DueTaskOutcome::NoRequest);
        }

        if let Some(schedule_id) = input.schedule_id
            && let Some(task) = self
                .task_repository
                .find_by_schedule_id_and_scheduled_at(schedule_id, input.scheduled_at)
                .await?
        {
            return Ok(DueTaskOutcome::AlreadyRecorded(task));
        }

        if input.schedule_id.is_none() {
            let recent = self.task_repository.list_recent(None, 100).await?;

            if let Some(task) = recent
                .iter()
                .find(|task| is_watch_task(task) && task.scheduled_at == Some(input.scheduled_at))
            {
                return Ok(DueTaskOutcome::AlreadyRecorded(task.clone()));
            }

            if input.skip_if_open_same_source
                && let Some(task) = recent
                    .into_iter()
                    .find(|task| is_watch_task(task) && !task.status.is_terminal())
            {
                return Ok(DueTaskOutcome::AlreadyRunning(task));
            }
        }

        let task = self
            .task_repository
            .create(None, input.schedule_id, Some(input.scheduled_at))
            .await?;

        self.message_repository
            .save(
                task.id,
                Role::User,
                vec![MessageContent::input_text(request)],
            )
            .await?;

        Ok(DueTaskOutcome::Started(task))
    }
}

fn is_watch_task(task: &Task) -> bool {
    task.schedule_id.is_none() && task.scheduled_at.is_some() && task.session_id.is_none()
}
