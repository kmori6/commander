CREATE TABLE schedules (
  id UUID PRIMARY KEY DEFAULT uuidv7(),
  title TEXT NOT NULL,
  request TEXT NOT NULL,
  cron TEXT NOT NULL,
  enabled BOOLEAN NOT NULL DEFAULT TRUE,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE schedule_runs (
  id UUID PRIMARY KEY DEFAULT uuidv7(),
  schedule_id UUID NOT NULL REFERENCES schedules(id) ON DELETE CASCADE,
  task_id UUID NOT NULL UNIQUE REFERENCES tasks(id) ON DELETE CASCADE,
  scheduled_at TIMESTAMPTZ NOT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  UNIQUE (schedule_id, scheduled_at)
);

CREATE INDEX idx_schedules_enabled ON schedules(enabled, id);
CREATE INDEX idx_schedule_runs_schedule_created ON schedule_runs(schedule_id, created_at, id);
CREATE INDEX idx_schedule_runs_task ON schedule_runs(task_id);
