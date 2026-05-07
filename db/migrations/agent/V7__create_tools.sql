CREATE TABLE tool_permissions (
  id UUID PRIMARY KEY DEFAULT uuidv7(),
  tool_name TEXT NOT NULL UNIQUE,
  mode TEXT NOT NULL CHECK (mode IN ('allow', 'ask', 'deny')),
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE tool_approvals (
  id UUID PRIMARY KEY DEFAULT uuidv7(),
  message_id UUID NOT NULL REFERENCES messages(id) ON DELETE CASCADE,
  call_id TEXT NOT NULL,
  status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'approved', 'rejected')),
  requested_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  resolved_at TIMESTAMPTZ,
  UNIQUE (message_id, call_id)
);

CREATE INDEX idx_tool_approvals_status_requested ON tool_approvals(status, requested_at, id);
CREATE INDEX idx_tool_approvals_message_call ON tool_approvals(message_id, call_id);
