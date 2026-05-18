use crate::application::error::watch_usecase_error::WatchUsecaseError;
use crate::domain::model::task::{Task, TaskSourceKind};
use crate::domain::repository::task_repository::{CreateTask, TaskRepository};
use crate::domain::repository::watch_repository::WatchRepository;
use crate::domain::service::instruction_service::InstructionService;
use chrono::{DateTime, Utc};
use std::sync::Arc;
use uuid::Uuid;

const RECENT_TASK_LIMIT: usize = 100;

pub struct WatchTaskStart {
    pub task: Task,
}

pub enum WatchExecutionOutcome {
    Started(WatchTaskStart),
    Skipped(WatchSkipReason),
}

pub enum WatchSkipReason {
    NoConfig,
    Disabled,
    NotDue,
    NoRequest,
    AlreadyRecorded,
    AlreadyRunning(Uuid),
}

pub struct WatchUsecase<T, W> {
    task_repository: T,
    watch_repository: W,
    instruction_service: Arc<InstructionService>,
}

impl<T, W> WatchUsecase<T, W>
where
    T: TaskRepository,
    W: WatchRepository,
{
    pub fn new(
        task_repository: T,
        watch_repository: W,
        instruction_service: Arc<InstructionService>,
    ) -> Self {
        Self {
            task_repository,
            watch_repository,
            instruction_service,
        }
    }

    pub async fn run_due(
        &self,
        now: DateTime<Utc>,
        window: chrono::Duration,
    ) -> Result<WatchExecutionOutcome, WatchUsecaseError> {
        let Some(config) = self.watch_repository.get().await? else {
            return Ok(WatchExecutionOutcome::Skipped(WatchSkipReason::NoConfig));
        };

        if !config.enabled {
            return Ok(WatchExecutionOutcome::Skipped(WatchSkipReason::Disabled));
        }

        let Some(scheduled_at) = config.due_time(now, window) else {
            return Ok(WatchExecutionOutcome::Skipped(WatchSkipReason::NotDue));
        };

        let Some(request) = self.instruction_service.build_watch_request() else {
            return Ok(WatchExecutionOutcome::Skipped(WatchSkipReason::NoRequest));
        };

        let recent = self
            .task_repository
            .list_recent(None, RECENT_TASK_LIMIT)
            .await?;

        if recent.iter().any(|task| {
            task.source_kind == TaskSourceKind::Watch && task.scheduled_at == Some(scheduled_at)
        }) {
            return Ok(WatchExecutionOutcome::Skipped(
                WatchSkipReason::AlreadyRecorded,
            ));
        }

        if let Some(task) = recent
            .iter()
            .find(|task| task.source_kind == TaskSourceKind::Watch && !task.status.is_terminal())
        {
            return Ok(WatchExecutionOutcome::Skipped(
                WatchSkipReason::AlreadyRunning(task.id),
            ));
        }

        let task = self
            .task_repository
            .create(CreateTask {
                request,
                session_id: None,
                source_kind: TaskSourceKind::Watch,
                source_message_id: None,
                source_schedule_id: None,
                source_tool_call_id: None,
                subagent_profile: None,
                parent_task_id: None,
                scheduled_at: Some(scheduled_at),
            })
            .await?;

        Ok(WatchExecutionOutcome::Started(WatchTaskStart { task }))
    }
}
