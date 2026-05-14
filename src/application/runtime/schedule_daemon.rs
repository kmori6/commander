use chrono::Utc;
use std::sync::Arc;
use std::time::Duration;
use tokio::time;

use crate::application::error::schedule_daemon_error::ScheduleDaemonError;
use crate::application::runtime::agent_runtime::AgentRuntime;
use crate::application::usecase::schedule_usecase::ScheduleExecutionOutcome;
use crate::application::usecase::schedule_usecase::ScheduleUsecase;
use crate::infrastructure::llm::llm_gateway::LlmGateway;
use crate::infrastructure::persistence::file_schedule_repository::FileScheduleRepository;
use crate::infrastructure::persistence::file_tool_permission_repository::FileToolPermissionRepository;
use crate::infrastructure::persistence::postgres_event_repository::PostgresEventRepository;
use crate::infrastructure::persistence::postgres_message_repository::PostgresMessageRepository;
use crate::infrastructure::persistence::postgres_task_repository::PostgresTaskRepository;
use crate::infrastructure::persistence::postgres_token_usage_repository::PostgresTokenUsageRepository;
use crate::infrastructure::persistence::postgres_tool_approval_repository::PostgresToolApprovalRepository;

const POLL_INTERVAL: Duration = Duration::from_secs(30);
const DUE_WINDOW: chrono::Duration = chrono::Duration::seconds(60);

type AppScheduleUsecase = ScheduleUsecase<FileScheduleRepository, PostgresTaskRepository>;

type AppAgentRuntime = AgentRuntime<
    LlmGateway,
    PostgresTaskRepository,
    PostgresMessageRepository,
    PostgresEventRepository,
    PostgresTokenUsageRepository,
    FileToolPermissionRepository,
    PostgresToolApprovalRepository,
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

    async fn tick(&self) -> Result<(), ScheduleDaemonError> {
        let schedules = self.schedule_usecase.list_enabled().await?;
        let now = Utc::now();

        for schedule in schedules {
            let Some(scheduled_at) = schedule.due_time(now, DUE_WINDOW) else {
                continue;
            };

            let task_id = match self
                .schedule_usecase
                .run_once_at(schedule.id, scheduled_at)
                .await?
            {
                ScheduleExecutionOutcome::Started(start) => start.task.id,
                ScheduleExecutionOutcome::AlreadyRecorded(_) => continue,
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
