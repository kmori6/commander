# Database

PostgreSQL database name: `agent`.

- Admin migrations (DB-level setup): [`db/migrations/admin`](../db/migrations/admin)
- Application migrations (tables/indexes): [`db/migrations/agent`](../db/migrations/agent)
- Memory search uses the `vector` PostgreSQL extension created by `V5__create_memory_index.sql`.

## Tables

### `chat_sessions`

One row per conversation thread.

| Column       | Type      | Description        |
| ------------ | --------- | ------------------ |
| `id`         | UUID PK   | Session identifier |
| `created_at` | timestamp |                    |
| `updated_at` | timestamp |                    |

### `chat_messages`

Ordered message history for a session.

| Column       | Type                      | Description                           |
| ------------ | ------------------------- | ------------------------------------- |
| `id`         | UUID PK                   |                                       |
| `session_id` | UUID FK → `chat_sessions` |                                       |
| `role`       | text                      | `system` / `user` / `assistant`       |
| `kind`       | text                      | `text` / `tool_call` / `tool_results` |
| `text`       | text                      | Message text                          |
| `payload`    | JSONB                     | Structured tool data                  |
| `created_at` | timestamp                 |                                       |

### `token_usages`

Token consumption per LLM call. Used for context management and usage tracking.

| Column               | Type                      | Description |
| -------------------- | ------------------------- | ----------- |
| `id`                 | UUID PK                   |             |
| `message_id`         | UUID FK → `chat_messages` |             |
| `model`              | text                      |             |
| `input_tokens`       | int                       |             |
| `output_tokens`      | int                       |             |
| `cache_read_tokens`  | int                       |             |
| `cache_write_tokens` | int                       |             |
| `created_at`         | timestamp                 |             |

### `tool_call_approvals`

Audit log of user approval/denial decisions for tool calls.

| Column         | Type                      | Description           |
| -------------- | ------------------------- | --------------------- |
| `id`           | UUID PK                   |                       |
| `session_id`   | UUID FK → `chat_sessions` |                       |
| `tool_call_id` | text                      |                       |
| `tool_name`    | text                      |                       |
| `arguments`    | JSONB                     |                       |
| `decision`     | text                      | `approved` / `denied` |
| `decided_at`   | timestamp                 |                       |

### `tool_execution_rules`

Persisted per-tool execution rules. Combined with each tool's default policy to produce the final execution decision.

| Column       | Type        | Description              |
| ------------ | ----------- | ------------------------ |
| `id`         | UUID PK     |                          |
| `tool_name`  | text UNIQUE |                          |
| `action`     | text        | `allow` / `ask` / `deny` |
| `created_at` | timestamp   |                          |
| `updated_at` | timestamp   |                          |

### `memory_index`

Semantic search index for journal memory files. Rows are replaced per path when a journal file is written.

| Column        | Type        | Description                         |
| ------------- | ----------- | ----------------------------------- |
| `path`        | text PK     | Workspace-relative memory file path |
| `chunk_index` | int PK      | Zero-based chunk number             |
| `content`     | text        | Indexed Markdown chunk              |
| `embedding`   | vector      | Embedding used for similarity search |
| `indexed_at`  | timestamptz | Index timestamp                     |

## Migration Order

1. `V1__create_chat_tables.sql`
2. `V2__create_token_usages.sql`
3. `V3__create_tool_call_approvals.sql`
4. `V4__create_tool_execution_rules.sql`
5. `V5__create_memory_index.sql`
