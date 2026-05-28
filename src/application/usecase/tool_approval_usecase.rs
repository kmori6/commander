use uuid::Uuid;

use crate::application::error::tool_approval_usecase_error::ToolApprovalUsecaseError;
use crate::domain::error::task_repository_error::TaskRepositoryError;
use crate::domain::model::task::TaskStatus;
use crate::domain::model::tool_call::{ToolApproval, ToolApprovalStatus};
use crate::domain::repository::task_repository::TaskRepository;
use crate::domain::repository::tool_approval_repository::ToolApprovalRepository;

pub struct ToolApprovalUsecase<R, T> {
    tool_approval_repository: R,
    task_repository: T,
}

impl<R, T> ToolApprovalUsecase<R, T>
where
    R: ToolApprovalRepository,
    T: TaskRepository,
{
    pub fn new(tool_approval_repository: R, task_repository: T) -> Self {
        Self {
            tool_approval_repository,
            task_repository,
        }
    }

    pub async fn list(
        &self,
        status: Option<ToolApprovalStatus>,
    ) -> Result<Vec<ToolApproval>, ToolApprovalUsecaseError> {
        self.tool_approval_repository
            .list(status)
            .await
            .map_err(Into::into)
    }

    pub async fn approve(&self, id: Uuid) -> Result<ToolApproval, ToolApprovalUsecaseError> {
        self.resolve_approval(id, ToolApprovalStatus::Approved)
            .await
    }

    pub async fn reject(&self, id: Uuid) -> Result<ToolApproval, ToolApprovalUsecaseError> {
        self.resolve_approval(id, ToolApprovalStatus::Rejected)
            .await
    }

    // recovery: task: awaiting_approval + approval: approved/rejected + no tool_call_output -> task: queued
    pub async fn recover_resolved_approvals(&self) -> Result<u64, ToolApprovalUsecaseError> {
        let task_ids = self.tool_approval_repository.ready_task_ids().await?;
        let count = task_ids.len() as u64;

        for task_id in task_ids {
            self.task_repository.resume_after_approval(task_id).await?;
        }

        Ok(count)
    }

    // task status: awaiting approval -> queued
    async fn resolve_approval(
        &self,
        id: Uuid,
        status: ToolApprovalStatus,
    ) -> Result<ToolApproval, ToolApprovalUsecaseError> {
        let approval = self.tool_approval_repository.resolve(id, status).await?;

        let task = self
            .task_repository
            .find_by_id(approval.task_id)
            .await?
            .ok_or(TaskRepositoryError::NotFound(approval.task_id))?;

        if task.status == TaskStatus::AwaitingApproval {
            self.task_repository.resume_after_approval(task.id).await?;
        }

        Ok(approval)
    }
}
