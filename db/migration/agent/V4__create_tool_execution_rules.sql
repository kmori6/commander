CREATE TABLE tool_execution_rules (
  id UUID PRIMARY KEY DEFAULT uuidv7(),
  tool_name TEXT NOT NULL UNIQUE,
  action TEXT NOT NULL CHECK (action IN ('allow', 'ask', 'deny')),
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_tool_execution_rules_action
  ON tool_execution_rules(action);
