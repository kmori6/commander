use std::{collections::HashMap, sync::Arc};

use crate::application::error::tool_service_error::ToolServiceError;
use crate::domain::model::tool_call::{
    ToolCall, ToolCallOutput, ToolPermission, ToolPermissionMode, ToolSpec,
};
use crate::domain::port::tool::Tool;
use crate::domain::repository::tool_permission_repository::ToolPermissionRepository;

pub struct ToolService<P> {
    tools: HashMap<String, Arc<dyn Tool>>,
    permission_repository: P,
}

impl<P> ToolService<P>
where
    P: ToolPermissionRepository,
{
    pub fn new(tools: Vec<Arc<dyn Tool>>, permission_repository: P) -> Self {
        let tools = tools
            .into_iter()
            .map(|t| (t.name().to_string(), t))
            .collect();
        Self {
            tools,
            permission_repository,
        }
    }

    pub fn list_tools(&self) -> Vec<ToolSpec> {
        self.tools.values().map(|t| t.spec()).collect()
    }

    pub async fn list_permissions(&self) -> Result<Vec<ToolPermission>, ToolServiceError> {
        self.permission_repository.list().await.map_err(Into::into)
    }

    pub async fn update_permission(
        &self,
        tool_name: &str,
        mode: ToolPermissionMode,
    ) -> Result<ToolPermission, ToolServiceError> {
        if !self.tools.contains_key(tool_name) {
            return Err(ToolServiceError::ToolNotFound(tool_name.to_string()));
        }
        self.permission_repository
            .upsert(tool_name, mode)
            .await
            .map_err(Into::into)
    }

    pub fn specs_for(
        &self,
        allowed_tools: Option<&[String]>,
        extra_specs: impl IntoIterator<Item = ToolSpec>,
    ) -> Vec<ToolSpec> {
        let mut specs = self.list_tools();
        specs.extend(extra_specs);
        if let Some(allowed) = allowed_tools {
            specs.retain(|s| allowed.iter().any(|t| t == &s.name));
        }
        specs
    }

    pub async fn execute(&self, call: ToolCall) -> Result<ToolCallOutput, ToolServiceError> {
        let tool = self
            .tools
            .get(&call.tool_name)
            .ok_or_else(|| ToolServiceError::ToolNotFound(call.tool_name.clone()))?;
        let output = tool.execute(call.arguments).await?;
        Ok(ToolCallOutput::success(call.call_id, output))
    }

    pub async fn permission_mode(
        &self,
        tool_name: &str,
        allowed_tools: Option<&[String]>,
    ) -> Result<ToolPermissionMode, ToolServiceError> {
        if let Some(allowed) = allowed_tools
            && !allowed.iter().any(|t| t == tool_name)
        {
            return Ok(ToolPermissionMode::Deny);
        }

        Ok(if let Some(p) = self.permission_repository.find(tool_name).await? {
            p.mode
        } else {
            self.tools
                .get(tool_name)
                .map(|t| t.default_permission())
                .unwrap_or(ToolPermissionMode::Deny)
        })
    }
}
