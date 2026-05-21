use crate::application::usecase::schedule_usecase::{DueTaskInput, ScheduleUsecase};
use crate::domain::repository::watch_repository::WatchRepository;
use crate::domain::service::instruction_service::InstructionService;
use crate::infrastructure::persistence::file_schedule_repository::FileScheduleRepository;
use crate::infrastructure::persistence::file_watch_repository::FileWatchRepository;
use crate::infrastructure::persistence::postgres_message_repository::PostgresMessageRepository;
use crate::infrastructure::persistence::postgres_task_repository::PostgresTaskRepository;
use crate::presentation::error::schedule_daemon_error::ScheduleDaemonError;
use chrono::Utc;
use std::sync::Arc;
use std::time::Duration;
use tokio::time;

const POLL_INTERVAL: Duration = Duration::from_secs(30);
const DUE_WINDOW: chrono::Duration = chrono::Duration::seconds(60);

type AppScheduleUsecase =
    ScheduleUsecase<FileScheduleRepository, PostgresTaskRepository, PostgresMessageRepository>;

pub struct ScheduleDaemon {
    schedule_usecase: Arc<AppScheduleUsecase>,
    watch_repository: FileWatchRepository,
    instruction_service: Arc<InstructionService>,
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
        }
    }

    pub async fn run(self) {
        let mut interval = time::interval(POLL_INTERVAL);

        loop {
            interval.tick().await;

            if let Err(err) = self.tick_schedules().await {
                log::warn!("schedule daemon tick failed: {err}");
            }

            if let Err(err) = self.tick_watch().await {
                log::warn!("watch daemon tick failed: {err}");
            }
        }
    }

    async fn tick_schedules(&self) -> Result<(), ScheduleDaemonError> {
        let schedules = self.schedule_usecase.list_enabled().await?;
        let now = Utc::now();

        for schedule in schedules {
            let Some(scheduled_at) = schedule.due_time(now, DUE_WINDOW) else {
                continue;
            };

            self.schedule_usecase
                .run_once_at(schedule.id, scheduled_at)
                .await?;
        }

        Ok(())
    }

    async fn tick_watch(&self) -> Result<(), ScheduleDaemonError> {
        let Some(config) = self.watch_repository.get().await? else {
            return Ok(());
        };

        if !config.enabled {
            return Ok(());
        }

        let Some(scheduled_at) = config.due_time(Utc::now(), DUE_WINDOW) else {
            return Ok(());
        };

        let Some(request) = self.instruction_service.build_watch_request() else {
            return Ok(());
        };

        self.schedule_usecase
            .run_due_task(DueTaskInput {
                request,
                schedule_id: None,
                scheduled_at,
                skip_if_open_same_source: true,
            })
            .await?;

        Ok(())
    }
}
