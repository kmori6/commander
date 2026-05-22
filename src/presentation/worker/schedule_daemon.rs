use crate::application::service::instruction_service::InstructionService;
use crate::application::usecase::schedule_usecase::ScheduleUsecase;
use crate::domain::repository::watch_repository::WatchRepository;
use crate::infrastructure::persistence::file_schedule_repository::FileScheduleRepository;
use crate::infrastructure::persistence::file_watch_repository::FileWatchRepository;
use crate::infrastructure::persistence::postgres_message_repository::PostgresMessageRepository;
use crate::infrastructure::persistence::postgres_task_repository::PostgresTaskRepository;
use crate::presentation::error::schedule_daemon_error::ScheduleDaemonError;
use chrono::{DateTime, Utc};
use std::sync::Arc;
use std::time::Duration;
use tokio::time;

const TICK_INTERVAL: Duration = Duration::from_secs(30);

type AppScheduleUsecase =
    ScheduleUsecase<FileScheduleRepository, PostgresTaskRepository, PostgresMessageRepository>;

pub struct ScheduleDaemon {
    schedule_usecase: Arc<AppScheduleUsecase>,
    watch_repository: FileWatchRepository,
    instruction_service: Arc<InstructionService>,
    last_tick_at: Option<DateTime<Utc>>,
}

impl ScheduleDaemon {
    pub fn new(
        schedule_usecase: Arc<AppScheduleUsecase>,
        watch_repository: FileWatchRepository,
        instruction_service: Arc<InstructionService>,
    ) -> Self {
        Self {
            schedule_usecase,
            watch_repository,
            instruction_service,
            last_tick_at: None,
        }
    }

    pub async fn run(mut self) {
        let mut interval = time::interval(TICK_INTERVAL);

        loop {
            interval.tick().await;

            if let Err(err) = self.tick().await {
                log::warn!("schedule daemon tick failed: {err}");
            }
        }
    }

    async fn tick(&mut self) -> Result<(), ScheduleDaemonError> {
        let now = Utc::now();
        let window = self
            .last_tick_at
            .map(|last_tick_at| now - last_tick_at)
            .unwrap_or_else(tick_window);

        self.tick_schedules(now, window).await?;
        self.tick_watch(now, window).await?;

        self.last_tick_at = Some(now);
        Ok(())
    }

    async fn tick_schedules(
        &self,
        now: DateTime<Utc>,
        window: chrono::Duration,
    ) -> Result<(), ScheduleDaemonError> {
        let schedules = self.schedule_usecase.list().await?;

        for schedule in schedules {
            let Some(scheduled_at) = schedule.due_time(now, window) else {
                continue;
            };

            self.schedule_usecase
                .run_once_at(schedule.id, scheduled_at)
                .await?;
        }

        Ok(())
    }

    async fn tick_watch(
        &self,
        now: DateTime<Utc>,
        window: chrono::Duration,
    ) -> Result<(), ScheduleDaemonError> {
        let Some(config) = self.watch_repository.get().await? else {
            return Ok(());
        };

        if !config.enabled {
            return Ok(());
        }

        let Some(scheduled_at) = config.due_time(now, window) else {
            return Ok(());
        };

        let Some(request) = self.instruction_service.build_watch_request() else {
            return Ok(());
        };

        self.schedule_usecase
            .run_due_task(request, None, scheduled_at)
            .await?;

        Ok(())
    }
}

fn tick_window() -> chrono::Duration {
    chrono::Duration::from_std(TICK_INTERVAL).expect("TICK_INTERVAL must fit in chrono::Duration")
}
