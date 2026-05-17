use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Semaphore;
use tokio::time::{self, MissedTickBehavior};

use crate::application::runtime::agent_runtime::AgentRuntime;
use crate::domain::error::task_repository_error::TaskRepositoryError;
use crate::domain::repository::task_repository::TaskRepository;
use crate::infrastructure::llm::llm_gateway::LlmGateway;
use crate::infrastructure::persistence::file_tool_permission_repository::FileToolPermissionRepository;
use crate::infrastructure::persistence::postgres_event_repository::PostgresEventRepository;
use crate::infrastructure::persistence::postgres_message_repository::PostgresMessageRepository;
use crate::infrastructure::persistence::postgres_task_repository::PostgresTaskRepository;
use crate::infrastructure::persistence::postgres_token_usage_repository::PostgresTokenUsageRepository;
use crate::infrastructure::persistence::postgres_tool_approval_repository::PostgresToolApprovalRepository;

type AppAgentRuntime = AgentRuntime<
    LlmGateway,
    PostgresTaskRepository,
    PostgresMessageRepository,
    PostgresEventRepository,
    PostgresTokenUsageRepository,
    FileToolPermissionRepository,
    PostgresToolApprovalRepository,
>;

pub struct TaskRunner {
    task_repository: PostgresTaskRepository,
    agent_runtime: Arc<AppAgentRuntime>,
    batch_size: usize,
    concurrency: Arc<Semaphore>,
    poll_interval: Duration,
}

impl TaskRunner {
    pub fn new(
        task_repository: PostgresTaskRepository,
        agent_runtime: Arc<AppAgentRuntime>,
        batch_size: usize,
        max_concurrency: usize,
        poll_interval: Duration,
    ) -> Self {
        Self {
            task_repository,
            agent_runtime,
            batch_size: batch_size.max(1),
            concurrency: Arc::new(Semaphore::new(max_concurrency.max(1))),
            poll_interval,
        }
    }

    pub async fn run(self) {
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
        let limit = self.batch_size.min(self.concurrency.available_permits());

        if limit == 0 {
            return Ok(());
        }

        let tasks = self.task_repository.claim_queued(limit).await?;

        for task in tasks {
            let task_id = task.id;
            let agent_runtime = self.agent_runtime.clone();
            let permit = self.concurrency.clone().acquire_owned().await;

            tokio::spawn(async move {
                let Ok(_permit) = permit else {
                    log::warn!("task runner semaphore closed");
                    return;
                };

                if let Err(err) = agent_runtime.run(task_id).await {
                    log::warn!("failed to run claimed task {task_id}: {err}");
                }
            });
        }

        Ok(())
    }
}
