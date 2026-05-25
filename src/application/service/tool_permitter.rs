use crate::application::error::tool_permitter_error::ToolPermitterError;
use crate::application::service::tool_executor::ToolExecutor;
use crate::domain::model::tool_call::{ToolApproval, ToolPermission, ToolPermissionMode};
use crate::domain::repository::tool_approval_repository::ToolApprovalRepository;
use crate::domain::repository::tool_permission_repository::ToolPermissionRepository;
use std::sync::Arc;
use uuid::Uuid;

pub struct ToolPermitter<P, A> {
    tool_executor: Arc<ToolExecutor>,
    permission_repository: P,
    approval_repository: A,
}

impl<P, A> ToolPermitter<P, A>
where
    P: ToolPermissionRepository,
    A: ToolApprovalRepository,
{
    pub fn new(
        tool_executor: Arc<ToolExecutor>,
        permission_repository: P,
        approval_repository: A,
    ) -> Self {
        Self {
            tool_executor,
            permission_repository,
            approval_repository,
        }
    }

    pub async fn list(&self) -> Result<Vec<ToolPermission>, ToolPermitterError> {
        self.permission_repository.list().await.map_err(Into::into)
    }

    pub async fn update(
        &self,
        tool_name: &str,
        mode: ToolPermissionMode,
    ) -> Result<ToolPermission, ToolPermitterError> {
        if !self.tool_executor.exists(tool_name) {
            return Err(ToolPermitterError::ToolNotFound(tool_name.to_string()));
        }

        self.permission_repository
            .upsert(tool_name, mode)
            .await
            .map_err(Into::into)
    }

    pub async fn mode(
        &self,
        tool_name: &str,
        allowed_tools: Option<&[String]>,
        allow_approval: bool,
    ) -> Result<ToolPermissionMode, ToolPermitterError> {
        if let Some(allowed_tools) = allowed_tools
            && !allowed_tools.iter().any(|tool| tool == tool_name)
        {
            return Ok(ToolPermissionMode::Deny);
        }

        let mode = if let Some(permission) = self.permission_repository.find(tool_name).await? {
            permission.mode
        } else {
            self.tool_executor
                .default_permission(tool_name)
                .unwrap_or(ToolPermissionMode::Deny)
        };

        if allow_approval {
            Ok(mode)
        } else {
            Ok(mode.without_approval())
        }
    }

    pub async fn request(
        &self,
        task_id: Uuid,
        message_id: Uuid,
        call_id: &str,
    ) -> Result<ToolApproval, ToolPermitterError> {
        self.approval_repository
            .create_pending(task_id, message_id, call_id)
            .await
            .map_err(Into::into)
    }

    pub async fn ready(&self, task_id: Uuid) -> Result<Vec<ToolApproval>, ToolPermitterError> {
        self.approval_repository
            .ready_for_task(task_id)
            .await
            .map_err(Into::into)
    }

    pub async fn ready_tasks(&self) -> Result<Vec<Uuid>, ToolPermitterError> {
        self.approval_repository
            .ready_task_ids()
            .await
            .map_err(Into::into)
    }
}
