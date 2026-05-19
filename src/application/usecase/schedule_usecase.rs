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

pub struct DueTaskInput {
    pub request: String,
    pub source_kind: TaskSourceKind,
    pub source_schedule_id: Option<Uuid>,
    pub scheduled_at: DateTime<Utc>,
    pub skip_if_open_same_source: bool,
}

pub enum DueTaskOutcome {
    Started(ScheduledTaskStart),
    AlreadyRecorded(Task),
    AlreadyRunning(Task),
    NoRequest,
}

pub struct ScheduledTaskStart {
    pub execution: ScheduleExecution,
    pub task: Task,
}

pub enum ScheduleExecutionOutcome {
    Started(ScheduledTaskStart),
    AlreadyRecorded(ScheduleExecution),
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

        match self
            .run_due_task(DueTaskInput {
                request: schedule.request,
                source_kind: TaskSourceKind::Schedule,
                source_schedule_id: Some(schedule_id),
                scheduled_at,
                skip_if_open_same_source: false,
            })
            .await?
        {
            DueTaskOutcome::Started(start) => Ok(start),
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

        let schedule = self
            .schedule_repository
            .find_by_id(schedule_id)
            .await?
            .ok_or(ScheduleRepositoryError::NotFound(schedule_id))?;

        match self
            .run_due_task(DueTaskInput {
                request: schedule.request,
                source_kind: TaskSourceKind::Schedule,
                source_schedule_id: Some(schedule_id),
                scheduled_at,
                skip_if_open_same_source: false,
            })
            .await?
        {
            DueTaskOutcome::Started(start) => Ok(ScheduleExecutionOutcome::Started(start)),
            DueTaskOutcome::AlreadyRecorded(task) => Ok(ScheduleExecutionOutcome::AlreadyRecorded(
                execution_from_task(schedule_id, &task),
            )),
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

        if let Some(schedule_id) = input.source_schedule_id
            && let Some(task) = self
                .task_repository
                .find_by_source_schedule_id_and_scheduled_at(schedule_id, input.scheduled_at)
                .await?
        {
            return Ok(DueTaskOutcome::AlreadyRecorded(task));
        }

        if input.source_kind == TaskSourceKind::Watch {
            let recent = self.task_repository.list_recent(None, 100).await?;

            if let Some(task) = recent.iter().find(|task| {
                task.source_kind == TaskSourceKind::Watch
                    && task.scheduled_at == Some(input.scheduled_at)
            }) {
                return Ok(DueTaskOutcome::AlreadyRecorded(task.clone()));
            }

            if input.skip_if_open_same_source
                && let Some(task) = recent.into_iter().find(|task| {
                    task.source_kind == TaskSourceKind::Watch && !task.status.is_terminal()
                })
            {
                return Ok(DueTaskOutcome::AlreadyRunning(task));
            }
        }

        let task = self
            .task_repository
            .create(CreateTask {
                request,
                session_id: None,
                source_kind: input.source_kind,
                source_message_id: None,
                source_schedule_id: input.source_schedule_id,
                source_tool_call_id: None,
                subagent_profile: None,
                parent_task_id: None,
                scheduled_at: Some(input.scheduled_at),
            })
            .await?;

        let execution = ScheduleExecution {
            id: task.id,
            schedule_id: input.source_schedule_id.unwrap_or(task.id),
            task_id: task.id,
            scheduled_at: input.scheduled_at,
            created_at: task.created_at,
        };

        Ok(DueTaskOutcome::Started(ScheduledTaskStart {
            execution,
            task,
        }))
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
