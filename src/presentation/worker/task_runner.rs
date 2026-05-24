use std::sync::Arc;
use std::time::Duration;
use tokio::time::{self, MissedTickBehavior};

use crate::application::error::task_usecase_error::TaskUsecaseError;
use crate::application::runtime::agent_runtime::AgentRuntime;
use crate::application::usecase::task_usecase::TaskUsecase;
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

pub struct TaskRunner {
    task_usecase: Arc<TaskUsecase<PostgresTaskRepository, PostgresMessageRepository>>,
    agent_runtime: Arc<AppAgentRuntime>,
    poll_interval: Duration,
}

impl TaskRunner {
    pub fn new(
        task_usecase: Arc<TaskUsecase<PostgresTaskRepository, PostgresMessageRepository>>,
        agent_runtime: Arc<AppAgentRuntime>,
        poll_interval: Duration,
    ) -> Self {
        Self {
            task_usecase,
            agent_runtime,
            poll_interval,
        }
    }

    pub async fn run(self) {
        if let Err(err) = self.task_usecase.recover_interrupted().await {
            log::warn!("task runner recovery failed: {err}");
        }

        // task: awaiting_approval & approved/rejected & no tool_call_output -> queued
        if let Err(err) = self.agent_runtime.recover_approvals().await {
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

    async fn tick(&self) -> Result<(), TaskUsecaseError> {
        let Some(task) = self.task_usecase.claim_queued(1).await?.into_iter().next() else {
            return Ok(());
        };

        if let Err(err) = self.agent_runtime.clone().run(task.id).await {
            log::warn!("failed to run claimed task {}: {err}", task.id);
        }

        Ok(())
    }
}
