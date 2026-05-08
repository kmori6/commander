use chrono::Utc;
use uuid::Uuid;

use crate::application::error::schedule_usecase_error::ScheduleUsecaseError;
use crate::domain::error::schedule_repository_error::ScheduleRepositoryError;
use crate::domain::model::schedule::{Schedule, ScheduleRun};
use crate::domain::model::session::SessionKind;
use crate::domain::model::task::Task;
use crate::domain::repository::schedule_repository::{
    CreateSchedule, ScheduleRepository, UpdateSchedule,
};
use crate::domain::repository::session_repository::SessionRepository;
use crate::domain::repository::task_repository::{CreateTask, TaskRepository};

pub struct ScheduleRunTask {
    pub schedule_run: ScheduleRun,
    pub task: Task,
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
        enabled: bool,
    ) -> Result<Schedule, ScheduleUsecaseError> {
        self.schedule_repository
            .create(CreateSchedule {
                title,
                request,
                cron,
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

    pub async fn run_now(
        &self,
        schedule_id: Uuid,
    ) -> Result<ScheduleRunTask, ScheduleUsecaseError> {
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

        let schedule_run = self
            .schedule_repository
            .create_run(schedule_id, task.id, Utc::now())
            .await?;

        Ok(ScheduleRunTask { schedule_run, task })
    }

    pub async fn list_runs(
        &self,
        schedule_id: Uuid,
    ) -> Result<Vec<ScheduleRun>, ScheduleUsecaseError> {
        if self
            .schedule_repository
            .find_by_id(schedule_id)
            .await?
            .is_none()
        {
            return Err(ScheduleRepositoryError::NotFound(schedule_id).into());
        }

        self.schedule_repository
            .list_runs(schedule_id)
            .await
            .map_err(Into::into)
    }
}
