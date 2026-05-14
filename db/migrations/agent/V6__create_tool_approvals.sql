CREATE TABLE tool_approvals (
  id UUID PRIMARY KEY DEFAULT uuidv7(),
  task_id UUID NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
  message_content_id UUID NOT NULL UNIQUE REFERENCES message_contents(id) ON DELETE CASCADE,
  status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'approved', 'rejected')),
  requested_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  resolved_at TIMESTAMPTZ
);

CREATE INDEX idx_tool_approvals_task_status_requested
  ON tool_approvals(task_id, status, requested_at, id);

CREATE INDEX idx_tool_approvals_status_requested
  ON tool_approvals(status, requested_at, id);
