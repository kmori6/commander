CREATE TABLE results (
  id UUID PRIMARY KEY DEFAULT uuidv7(),
  task_id UUID NOT NULL UNIQUE REFERENCES tasks(id) ON DELETE CASCADE,
  status TEXT NOT NULL CHECK (status IN ('success', 'failure', 'cancelled')),
  output TEXT NOT NULL DEFAULT '',
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_results_task ON results(task_id);
