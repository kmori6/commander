use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Utc};
use chrono_tz::Tz;
use cron::Schedule as CronSchedule;
use tokio::time;

use crate::application::runtime::agent_runtime::AgentRuntime;
use crate::application::usecase::schedule_usecase::{ScheduleRunOutcome, ScheduleUsecase};
use crate::infrastructure::llm::llm_gateway::LlmGateway;
use crate::infrastructure::persistence::{
    postgres_event_repository::PostgresEventRepository,
    postgres_message_repository::PostgresMessageRepository,
    postgres_schedule_repository::PostgresScheduleRepository,
    postgres_session_repository::PostgresSessionRepository,
    postgres_task_repository::PostgresTaskRepository,
    postgres_task_result_repository::PostgresTaskResultRepository,
    postgres_token_usage_repository::PostgresTokenUsageRepository,
    postgres_tool_approval_repository::PostgresToolApprovalRepository,
    postgres_tool_permission_repository::PostgresToolPermissionRepository,
};

const POLL_INTERVAL: Duration = Duration::from_secs(30);
const DUE_WINDOW: chrono::Duration = chrono::Duration::seconds(60);

type AppScheduleUsecase =
    ScheduleUsecase<PostgresScheduleRepository, PostgresSessionRepository, PostgresTaskRepository>;

type AppAgentRuntime = AgentRuntime<
    LlmGateway,
    PostgresTaskRepository,
    PostgresMessageRepository,
    PostgresTaskResultRepository,
    PostgresEventRepository,
    PostgresTokenUsageRepository,
    PostgresToolPermissionRepository,
    PostgresToolApprovalRepository,
    PostgresSessionRepository,
>;

pub struct ScheduleDaemon {
    schedule_usecase: Arc<AppScheduleUsecase>,
    agent_runtime: Arc<AppAgentRuntime>,
}

impl ScheduleDaemon {
    pub fn new(
        schedule_usecase: Arc<AppScheduleUsecase>,
        agent_runtime: Arc<AppAgentRuntime>,
    ) -> Self {
        Self {
            schedule_usecase,
            agent_runtime,
        }
    }

    pub async fn run(self) {
        let mut interval = time::interval(POLL_INTERVAL);

        loop {
            interval.tick().await;

            if let Err(err) = self.tick().await {
                log::warn!("schedule daemon tick failed: {err}");
            }
        }
    }

    async fn tick(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let schedules = self.schedule_usecase.list_enabled().await?;
        let now = Utc::now();

        for schedule in schedules {
            let Some(scheduled_at) = due_time(&schedule.cron, &schedule.timezone, now)? else {
                continue;
            };

            let task_id = match self
                .schedule_usecase
                .run_once_at(schedule.id, scheduled_at)
                .await?
            {
                ScheduleRunOutcome::Started(run_task) => run_task.task.id,
                ScheduleRunOutcome::AlreadyRan(_) => continue,
            };
            let agent_runtime = self.agent_runtime.clone();

            tokio::spawn(async move {
                if let Err(err) = agent_runtime.run(task_id, None).await {
                    log::warn!("failed to run scheduled task {task_id}: {err}");
                }
            });
        }

        Ok(())
    }
}

fn due_time(
    cron: &str,
    timezone: &str,
    now: DateTime<Utc>,
) -> Result<Option<DateTime<Utc>>, Box<dyn std::error::Error + Send + Sync>> {
    let timezone = timezone.trim().parse::<Tz>().map_err(|err| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("invalid timezone {timezone}: {err}"),
        )
    })?;
    let expression = normalize_cron_expression(cron);
    let schedule = CronSchedule::from_str(&expression)?;

    let now_local = now.with_timezone(&timezone);
    let from_local = now_local - DUE_WINDOW;
    let scheduled_at = schedule
        .after(&from_local)
        .next()
        .filter(|scheduled_at| *scheduled_at <= now_local)
        .map(|scheduled_at| scheduled_at.with_timezone(&Utc));

    Ok(scheduled_at)
}

fn normalize_cron_expression(cron: &str) -> String {
    let cron = cron.trim();

    if cron.starts_with('@') {
        return cron.to_string();
    }

    match cron.split_whitespace().count() {
        5 => format!("0 {cron}"),
        _ => cron.to_string(),
    }
}
