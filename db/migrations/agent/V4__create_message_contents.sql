CREATE TABLE message_contents (
  id UUID PRIMARY KEY DEFAULT uuidv7(),
  message_id UUID NOT NULL REFERENCES messages(id) ON DELETE CASCADE,
  content_index INTEGER NOT NULL CHECK (content_index >= 0),
  type TEXT NOT NULL CHECK (type IN (
    'input_text',
    'output_text',
    'tool_call',
    'tool_call_output'
  )),
  text TEXT,
  call_id TEXT,
  tool_name TEXT,
  arguments JSONB,
  output JSONB,
  output_status TEXT CHECK (output_status IN ('success', 'error')),
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  UNIQUE (message_id, content_index)
);

CREATE INDEX idx_message_contents_message_index
  ON message_contents(message_id, content_index);

CREATE INDEX idx_message_contents_tool_call
  ON message_contents(message_id, call_id)
  WHERE call_id IS NOT NULL;
