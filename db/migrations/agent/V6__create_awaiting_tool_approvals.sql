CREATE TABLE awaiting_tool_approvals (
  session_id UUID PRIMARY KEY REFERENCES chat_sessions(id) ON DELETE CASCADE,
  assistant_message_id UUID NOT NULL REFERENCES chat_messages(id) ON DELETE CASCADE,
  tool_call_id TEXT NOT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

  UNIQUE (assistant_message_id, tool_call_id)
);

CREATE INDEX idx_awaiting_tool_approvals_created
  ON awaiting_tool_approvals(created_at DESC);
