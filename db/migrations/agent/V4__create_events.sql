CREATE TABLE events (
  id UUID PRIMARY KEY DEFAULT uuidv7(),
  task_id UUID NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
  event_type TEXT NOT NULL,
  payload JSONB NOT NULL DEFAULT '{}'::jsonb,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_events_task_created ON events(task_id, created_at, id);
CREATE INDEX idx_events_type_created ON events(event_type, created_at, id);
