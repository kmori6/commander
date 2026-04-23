# Database

This document is a short memo for the current database layout in `commander`.

## Overview

- PostgreSQL database name: `agent`
- Admin migrations: [`db/migration/admin`](../db/migration/admin)
- Application migrations: [`db/migration/agent`](../db/migration/agent)

`admin` is for PostgreSQL-level setup such as creating the `agent` database.
`agent` is for tables and indexes inside the `agent` database.

## Tables

### `chat_sessions`

- One row per chat session
- Primary key: `id UUID`
- Timestamps:
  - `created_at`
  - `updated_at`

This table is the container for a conversation thread.

### `chat_messages`

- One row per message in a session
- Primary key: `id UUID`
- Foreign key: `session_id -> chat_sessions.id`
- Core fields:
  - `role`
  - `kind`
  - `text`
  - `payload`
  - `created_at`

This table stores the ordered message history for a session.
`kind` separates plain text from tool-related messages.
`payload` stores structured tool data as `JSONB` when needed.

Message kinds:

- `text`
  - Normal system, user, or assistant text.
- `tool_call`
  - Assistant message containing one or more tool calls returned by the LLM.
- `tool_results`
  - Tool result message returned to the LLM. This can include successful tool
    results, execution errors, blocked tool calls, denied tool calls, or skipped
    tool calls.

### `token_usages`

- One row per recorded LLM usage event
- Primary key: `id UUID`
- Foreign key: `message_id -> chat_messages.id`
- Core fields:
  - `model`
  - `input_tokens`
  - `output_tokens`
  - `cache_read_tokens`
  - `cache_write_tokens`
  - `created_at`

This table stores usage metadata for messages produced by LLM calls.
It is used by context management and cost/usage tracking.

### `tool_call_approvals`

- One row per explicit user approval or denial decision
- Primary key: `id UUID`
- Foreign key: `session_id -> chat_sessions.id`
- Core fields:
  - `tool_call_id`
  - `tool_name`
  - `arguments`
  - `decision`
  - `decided_at`

`decision` is constrained to:

- `approved`
- `denied`

This table is an audit log for user decisions about tool execution.
It does not represent the current execution policy for a tool. It records what
the user decided for a specific tool call at a specific point in a session.

### `tool_execution_rules`

- One row per persisted tool execution rule
- Primary key: `id UUID`
- Unique key: `tool_name`
- Core fields:
  - `tool_name`
  - `action`
  - `created_at`
  - `updated_at`

`action` is constrained to:

- `allow`
- `ask`
- `deny`

This table stores the current default execution rule for a tool name.
The domain model combines this persisted action with the tool's own
`ToolExecutionPolicy` to produce a final `ToolExecutionDecision`.

Important behavior:

- `deny` blocks execution and returns an error tool result to the LLM.
- `ask` pauses the agent loop and requests user approval.
- `allow` lets the tool run automatically unless the tool declares
  `ConfirmEveryTime`.
- `ConfirmEveryTime` remains approval-gated even when the stored rule is
  `allow`.

## Migration Order

Application migrations currently create the tables in this order:

1. `V1__create_chat_tables.sql`
2. `V2__create_token_usages.sql`
3. `V3__create_tool_call_approvals.sql`
4. `V4__create_tool_execution_rules.sql`
