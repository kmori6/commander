CREATE EXTENSION IF NOT EXISTS vector;

CREATE TABLE memory_index (
  path TEXT NOT NULL,
  chunk_index INTEGER NOT NULL CHECK (chunk_index >= 0),
  content TEXT NOT NULL,
  embedding vector NOT NULL,
  indexed_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

  PRIMARY KEY (path, chunk_index)
);
