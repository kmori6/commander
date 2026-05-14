CREATE TABLE tasks (
  id UUID PRIMARY KEY DEFAULT uuidv7(),
  session_id UUID REFERENCES sessions(id) ON DELETE SET NULL,
  source_kind TEXT NOT NULL CHECK (source_kind IN ('chat', 'schedule', 'task', 'manual')),
  source_message_id UUID,
  source_schedule_id UUID,
  parent_task_id UUID REFERENCES tasks(id) ON DELETE SET NULL,
  scheduled_at TIMESTAMPTZ,
  request TEXT NOT NULL,
  status TEXT NOT NULL DEFAULT 'queued' CHECK (
    status IN (
      'queued',
      'running',
      'awaiting_approval',
      'completed',
      'failed',
      'cancel_requested',
      'cancelled'
    )
  ),
  output TEXT NOT NULL DEFAULT '',
  error TEXT,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  started_at TIMESTAMPTZ,
  finished_at TIMESTAMPTZ
);

CREATE INDEX idx_tasks_session_created
  ON tasks(session_id, created_at, id)
  WHERE session_id IS NOT NULL;

CREATE INDEX idx_tasks_status_created
  ON tasks(status, created_at, id);

CREATE INDEX idx_tasks_source_message
  ON tasks(source_message_id)
  WHERE source_message_id IS NOT NULL;

CREATE INDEX idx_tasks_source_schedule
  ON tasks(source_schedule_id, scheduled_at, id)
  WHERE source_schedule_id IS NOT NULL;

CREATE INDEX idx_tasks_parent
  ON tasks(parent_task_id)
  WHERE parent_task_id IS NOT NULL;

CREATE UNIQUE INDEX idx_tasks_schedule_scheduled_at_unique
  ON tasks(source_schedule_id, scheduled_at)
  WHERE source_kind = 'schedule'
    AND source_schedule_id IS NOT NULL
    AND scheduled_at IS NOT NULL;
