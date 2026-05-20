CREATE TABLE messages (
  id UUID PRIMARY KEY DEFAULT uuidv7(),
  task_id UUID NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
  role TEXT NOT NULL CHECK (role IN ('system', 'user', 'assistant')),
  contents JSONB NOT NULL DEFAULT '[]'::jsonb,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  CHECK (jsonb_typeof(contents) = 'array')
);

CREATE INDEX idx_messages_task_created
  ON messages(task_id, created_at, id);
