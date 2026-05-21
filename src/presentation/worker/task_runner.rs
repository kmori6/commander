use std::sync::Arc;
use std::time::Duration;
use tokio::time::{self, MissedTickBehavior};

use crate::application::runtime::agent_runtime::AgentRuntime;
use crate::domain::error::task_repository_error::TaskRepositoryError;
use crate::domain::repository::task_repository::TaskRepository;
use crate::infrastructure::llm::llm_gateway::LlmGateway;
use crate::infrastructure::persistence::file_tool_permission_repository::FileToolPermissionRepository;
use crate::infrastructure::persistence::postgres_message_repository::PostgresMessageRepository;
use crate::infrastructure::persistence::postgres_task_repository::PostgresTaskRepository;
use crate::infrastructure::persistence::postgres_tool_approval_repository::PostgresToolApprovalRepository;

type AppAgentRuntime = AgentRuntime<
    LlmGateway,
    PostgresTaskRepository,
    PostgresMessageRepository,
    FileToolPermissionRepository,
    PostgresToolApprovalRepository,
>;

const APPROVAL_RECOVERY_LIMIT: usize = 20;

pub struct TaskRunner {
    task_repository: PostgresTaskRepository,
    agent_runtime: Arc<AppAgentRuntime>,
    poll_interval: Duration,
}

impl TaskRunner {
    pub fn new(
        task_repository: PostgresTaskRepository,
        agent_runtime: Arc<AppAgentRuntime>,
        poll_interval: Duration,
    ) -> Self {
        Self {
            task_repository,
            agent_runtime,
            poll_interval,
        }
    }

    pub async fn run(self) {
        if let Err(err) = self.recover_interrupted().await {
            log::warn!("task runner recovery failed: {err}");
        }

        if let Err(err) = self
            .agent_runtime
            .recover_approvals(APPROVAL_RECOVERY_LIMIT)
            .await
        {
            log::warn!("approval recovery failed: {err}");
        }

        let mut interval = time::interval(self.poll_interval);
        interval.set_missed_tick_behavior(MissedTickBehavior::Delay);

        loop {
            interval.tick().await;

            if let Err(err) = self.tick().await {
                log::warn!("task runner tick failed: {err}");
            }
        }
    }

    async fn tick(&self) -> Result<(), TaskRepositoryError> {
        let Some(task) = self
            .task_repository
            .claim_queued(1)
            .await?
            .into_iter()
            .next()
        else {
            return Ok(());
        };

        if let Err(err) = self.agent_runtime.clone().run(task.id).await {
            log::warn!("failed to run claimed task {}: {err}", task.id);
        }

        Ok(())
    }

    async fn recover_interrupted(&self) -> Result<(), TaskRepositoryError> {
        let count = self.task_repository.requeue_running().await?;

        if count > 0 {
            log::warn!("requeued {count} interrupted running task(s)");
        }

        Ok(())
    }
}
