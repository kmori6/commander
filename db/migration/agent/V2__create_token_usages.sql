CREATE TABLE token_usages (
  id UUID PRIMARY KEY DEFAULT uuidv7(),
  message_id UUID NOT NULL REFERENCES chat_messages(id) ON DELETE CASCADE,
  model TEXT NOT NULL,
  input_tokens BIGINT NOT NULL CHECK (input_tokens >= 0),
  output_tokens BIGINT NOT NULL CHECK (output_tokens >= 0),
  cache_read_tokens BIGINT NOT NULL DEFAULT 0 CHECK (cache_read_tokens >= 0),
  cache_write_tokens BIGINT NOT NULL DEFAULT 0 CHECK (cache_write_tokens >= 0),
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_token_usages_message_created
  ON token_usages(message_id, created_at, id);
