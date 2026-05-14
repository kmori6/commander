CREATE TABLE sessions (
  id UUID PRIMARY KEY DEFAULT uuidv7(),
  title TEXT,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_sessions_updated
  ON sessions(updated_at DESC, id);
