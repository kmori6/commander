CREATE EXTENSION IF NOT EXISTS vector;

CREATE TABLE memory_index (
  path TEXT NOT NULL,
  chunk_index INTEGER NOT NULL CHECK (chunk_index >= 0),
  content TEXT NOT NULL,
  embedding vector NOT NULL,
  indexed_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  PRIMARY KEY (path, chunk_index)
);

CREATE INDEX idx_memory_index_path ON memory_index(path);
CREATE INDEX idx_memory_index_indexed_at ON memory_index(indexed_at);
