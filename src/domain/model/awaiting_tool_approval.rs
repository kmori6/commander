use uuid::Uuid;

pub struct AwaitingToolApproval {
    pub session_id: Uuid,
    pub assistant_message_id: Uuid,
    pub tool_call_id: String,
}
