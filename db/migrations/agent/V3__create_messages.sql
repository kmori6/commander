CREATE TABLE messages (
  id UUID PRIMARY KEY DEFAULT uuidv7(),
  task_id UUID NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
  role TEXT NOT NULL CHECK (role IN ('system', 'user', 'assistant')),
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_messages_task_created
  ON messages(task_id, created_at, id);

ALTER TABLE tasks
  ADD CONSTRAINT tasks_source_message_id_fkey
  FOREIGN KEY (source_message_id)
  REFERENCES messages(id)
  ON DELETE SET NULL;
