CREATE TABLE sessions (
  id UUID PRIMARY KEY DEFAULT uuidv7(),
  kind TEXT NOT NULL CHECK (kind IN ('chat', 'task')),
  title TEXT,
  status TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'closed')),
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_sessions_kind_updated
  ON sessions(kind, updated_at DESC, id);
