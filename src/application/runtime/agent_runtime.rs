use crate::domain::error::agent_error::AgentError;
use crate::domain::error::tool_error::ToolError;
use crate::domain::model::message::{Message, MessageContent};
use crate::domain::model::role::Role;
use crate::domain::model::tool_call::ToolCall;
use crate::domain::model::tool_call_output::ToolCallOutput;
use crate::domain::model::tool_execution_decision::ToolExecutionDecision;
use crate::domain::model::tool_execution_policy::ToolExecutionPolicy;
use crate::domain::port::llm_provider::{LlmProvider, LlmResponse};
use crate::domain::service::tool_service::ToolService;

const DEFAULT_MODEL: &str = "global.anthropic.claude-sonnet-4-6";

pub struct AgentRuntime<L> {
    llm_provider: L,
    tool_service: ToolService,
    model: String,
}

impl<L: LlmProvider> AgentRuntime<L> {
    pub fn new(llm_provider: L, tool_service: ToolService) -> Self {
        Self {
            llm_provider,
            tool_service,
            model: DEFAULT_MODEL.to_string(),
        }
    }

    pub fn model(&self) -> &str {
        &self.model
    }

    pub async fn llm_step(
        &self,
        instruction: String,
        messages: Vec<Message>,
    ) -> Result<LlmResponse, AgentError> {
        let instructions = Message::new(
            Role::System,
            vec![MessageContent::InputText { text: instruction }],
        )?;

        let mut llm_messages = Vec::with_capacity(messages.len() + 1);
        llm_messages.push(instructions);
        llm_messages.extend(messages);

        self.llm_provider
            .response_with_tool(llm_messages, self.tool_service.specs(), &self.model)
            .await
            .map_err(AgentError::LlmProvider)
    }

    pub async fn execute_tool_call(&self, tool_call: ToolCall) -> ToolCallOutput {
        let call_id = tool_call.call_id.clone();

        match self.tool_service.execute(tool_call).await {
            Ok(output) => output,
            Err(err) => ToolCallOutput::error_message(call_id, err.to_string()),
        }
    }

    pub async fn decide_tool_call(
        &self,
        tool_call: &ToolCall,
    ) -> Result<ToolExecutionDecision, ToolError> {
        self.tool_service.decide_execution(tool_call).await
    }

    pub fn check_tool_policy(
        &self,
        tool_call: &ToolCall,
    ) -> Result<ToolExecutionPolicy, ToolError> {
        self.tool_service.check_execution_policy(tool_call)
    }
}
