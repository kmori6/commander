CREATE TABLE tool_approvals (
  id UUID PRIMARY KEY DEFAULT uuidv7(),
  task_id UUID NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
  message_id UUID NOT NULL REFERENCES messages(id) ON DELETE CASCADE,
  call_id TEXT NOT NULL,
  status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'approved', 'rejected')),
  requested_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  resolved_at TIMESTAMPTZ,
  UNIQUE (message_id, call_id)
);

CREATE INDEX idx_tool_approvals_task_status_requested
  ON tool_approvals(task_id, status, requested_at, id);

CREATE INDEX idx_tool_approvals_status_requested
  ON tool_approvals(status, requested_at, id);
