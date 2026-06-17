CREATE TABLE tasks (
  id UUID PRIMARY KEY DEFAULT uuidv7(),
  session_id UUID REFERENCES sessions(id) ON DELETE SET NULL,
  schedule_id UUID,
  scheduled_at TIMESTAMPTZ,
  status TEXT NOT NULL DEFAULT 'queued' CHECK (
    status IN ('queued', 'running', 'completed', 'failed', 'cancelled')
  ),
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

CREATE INDEX idx_tasks_schedule
  ON tasks(schedule_id, scheduled_at, id)
  WHERE schedule_id IS NOT NULL;

CREATE UNIQUE INDEX idx_tasks_schedule_scheduled_at_unique
  ON tasks(schedule_id, scheduled_at)
  WHERE schedule_id IS NOT NULL
    AND scheduled_at IS NOT NULL;
