use std::sync::Arc;
use std::time::Duration;
use tokio::time::{self, MissedTickBehavior};

use crate::application::error::task_usecase_error::TaskUsecaseError;
use crate::application::runtime::agent_runtime::AgentRuntime;
use crate::application::usecase::task_usecase::TaskUsecase;
use crate::application::usecase::tool_approval_usecase::ToolApprovalUsecase;
use crate::domain::port::llm_provider::LlmProvider;
use crate::domain::repository::message_repository::MessageRepository;
use crate::domain::repository::subagent_repository::SubagentRepository;
use crate::domain::repository::task_repository::TaskRepository;
use crate::domain::repository::tool_approval_repository::ToolApprovalRepository;
use crate::domain::repository::tool_permission_repository::ToolPermissionRepository;

pub struct TaskRunner<L, T, M, S, P, A> {
    task_usecase: Arc<TaskUsecase<T, M>>,
    tool_approval_usecase: Arc<ToolApprovalUsecase<A, T>>,
    agent_runtime: Arc<AgentRuntime<L, T, M, S, P, A>>,
    poll_interval: Duration,
}

impl<L, T, M, S, P, A> TaskRunner<L, T, M, S, P, A>
where
    L: LlmProvider,
    T: TaskRepository,
    M: MessageRepository,
    S: SubagentRepository,
    P: ToolPermissionRepository,
    A: ToolApprovalRepository,
{
    pub fn new(
        task_usecase: Arc<TaskUsecase<T, M>>,
        tool_approval_usecase: Arc<ToolApprovalUsecase<A, T>>,
        agent_runtime: Arc<AgentRuntime<L, T, M, S, P, A>>,
        poll_interval: Duration,
    ) -> Self {
        Self {
            task_usecase,
            tool_approval_usecase,
            agent_runtime,
            poll_interval,
        }
    }

    pub async fn run(self) {
        if let Err(err) = self.task_usecase.recover_interrupted().await {
            log::warn!("task runner recovery failed: {err}");
        }

        if let Err(err) = self
            .tool_approval_usecase
            .recover_resolved_approvals()
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
