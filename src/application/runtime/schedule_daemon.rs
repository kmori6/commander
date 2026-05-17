use crate::application::error::schedule_daemon_error::ScheduleDaemonError;
use crate::application::usecase::schedule_usecase::ScheduleUsecase;
use crate::application::usecase::watch_usecase::WatchUsecase;
use crate::infrastructure::persistence::file_schedule_repository::FileScheduleRepository;
use crate::infrastructure::persistence::file_watch_repository::FileWatchRepository;
use crate::infrastructure::persistence::postgres_task_repository::PostgresTaskRepository;
use chrono::Utc;
use std::sync::Arc;
use std::time::Duration;
use tokio::time;

const POLL_INTERVAL: Duration = Duration::from_secs(30);
const DUE_WINDOW: chrono::Duration = chrono::Duration::seconds(60);

type AppScheduleUsecase = ScheduleUsecase<FileScheduleRepository, PostgresTaskRepository>;
type AppWatchUsecase = WatchUsecase<PostgresTaskRepository, FileWatchRepository>;

pub struct ScheduleDaemon {
    schedule_usecase: Arc<AppScheduleUsecase>,
    watch_usecase: Arc<AppWatchUsecase>,
}

impl ScheduleDaemon {
    pub fn new(
        schedule_usecase: Arc<AppScheduleUsecase>,
        watch_usecase: Arc<AppWatchUsecase>,
    ) -> Self {
        Self {
            schedule_usecase,
            watch_usecase,
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
        self.watch_usecase.run_due(Utc::now(), DUE_WINDOW).await?;
        Ok(())
    }
}
