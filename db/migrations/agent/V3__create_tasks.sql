CREATE TABLE tasks (
  id UUID PRIMARY KEY DEFAULT uuidv7(),
  request TEXT NOT NULL,
  status TEXT NOT NULL DEFAULT 'queued' CHECK (
    status IN ('queued', 'running', 'awaiting_approval', 'completed', 'failed', 'cancel_requested', 'cancelled')
  ),
  session_id UUID NOT NULL REFERENCES sessions(id),
  source_message_id UUID REFERENCES messages(id) ON DELETE SET NULL,
  parent_task_id UUID REFERENCES tasks(id) ON DELETE SET NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  started_at TIMESTAMPTZ,
  finished_at TIMESTAMPTZ
);

CREATE INDEX idx_tasks_status_created ON tasks(status, created_at, id);
CREATE INDEX idx_tasks_session ON tasks(session_id);
CREATE INDEX idx_tasks_source_message ON tasks(source_message_id);
CREATE INDEX idx_tasks_parent ON tasks(parent_task_id);
