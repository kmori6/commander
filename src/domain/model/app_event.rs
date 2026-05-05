use crate::domain::model::token_usage::TokenUsage;
use crate::domain::model::tool_approval::ToolApprovalResponse;
use crate::domain::model::tool_call_output::ToolCallOutputStatus;
use crate::domain::model::tool_execution_policy::ToolExecutionPolicy;
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub enum AppEvent {
    AgentTurnStarted {
        session_id: Uuid,
    },
    AgentTurnCompleted {
        session_id: Uuid,
    },
    AgentTurnFailed {
        session_id: Uuid,
        reason: String,
    },
    LlmStarted {
        session_id: Uuid,
    },
    LlmFinished {
        session_id: Uuid,
    },
    LlmUsageRecorded {
        session_id: Uuid,
        message_id: Uuid,
        usage: TokenUsage,
    },
    ToolCallStarted {
        session_id: Uuid,
        call_id: String,
        tool_name: String,
        arguments: Value,
    },
    ToolCallFinished {
        session_id: Uuid,
        call_id: String,
        tool_name: String,
        output: Value,
        status: ToolCallOutputStatus,
    },
    AssistantMessageCreated {
        session_id: Uuid,
        message_id: Uuid,
        content: String,
    },
    ToolCallApprovalRequested {
        session_id: Uuid,
        call_id: String,
        tool_name: String,
        arguments: Value,
        policy: ToolExecutionPolicy,
    },
    ToolCallApprovalResolved {
        session_id: Uuid,
        call_id: String,
        tool_name: String,
        decision: ToolApprovalResponse,
    },
}
