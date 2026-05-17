# 🦉 Commander

AI agent for R&D software engineering work

[![Test](https://github.com/kmori6/commander/actions/workflows/test.yaml/badge.svg)](https://github.com/kmori6/commander/actions/workflows/test.yaml)
[![Lint](https://github.com/kmori6/commander/actions/workflows/lint.yaml/badge.svg)](https://github.com/kmori6/commander/actions/workflows/lint.yaml)

## Prerequisites

- [Rust](https://www.rust-lang.org/tools/install) 1.85+
- [Docker](https://docs.docker.com/get-docker/)
- [markitdown](https://github.com/microsoft/markitdown) (`pip install markitdown`)
- AWS account with Bedrock access
- [Tavily](https://tavily.com/) API key

## Setup

1. Start PostgreSQL and run migrations:

   ```bash
   docker compose up -d postgres flyway-admin flyway-agent
   ```

2. Copy `.env.sample` to `.env` and fill in your credentials:

   ```bash
   cp .env.sample .env
   ```

3. Fill in your AWS credentials and other API keys in `.env` (see `.env.sample` for the full list of variables).

## Installation

```bash
cargo install --path .
```

## Usage

### Server

Start the local HTTP/SSE server.

```bash
commander serve
```

By default, the server listens on `0.0.0.0:3000`.

```bash
commander serve --addr 127.0.0.1:3000
```

### Chat

Start the server-backed chat CLI.

```bash
commander chat
```

Use a different server or resume an existing session:

```bash
commander chat --base-url http://localhost:3000
commander chat --session-id <uuid>
```

| Command                           | Description                    |
| --------------------------------- | ------------------------------ |
| `/new`                            | Start a new session            |
| `/approve`                        | Approve pending tool execution |
| `/deny`                           | Deny pending tool execution    |
| `/tools`                          | Show tool execution status     |
| `/tool <tool> <allow\|ask\|deny>` | Set a tool execution rule      |
| `/usage`                          | Show session token usage       |
| `/attach <files...>`              | Stage files to attach          |
| `/files`                          | Show staged files              |
| `/detach <index\|all>`            | Remove staged files            |
| `/exit`                           | Quit                           |

## Features

### Skill

Skills are reusable workspace instructions for specialized workflows.
Add a skill as `~/.commander/workspace/skills/<name>/SKILL.md`:

```markdown
---
name: skill-name
description: A description of what this skill does and when to use it.
---

# Instructions

...
```

Reference: [Agent Skills Specification](https://agentskills.io/specification)

### Memory

Memory stores workspace context in two files; edit them directly to add or update memory:

- Long-term: `~/.commander/workspace/memory/MEMORY.md`
- Short-term: `~/.commander/workspace/memory/journals/YYYY-MM-DD.md`

### Schedule

Schedule creates recurring tasks from saved requests.
Create one with the HTTP API:

```bash
curl -X POST http://localhost:3000/v1/schedules \
  -H 'Content-Type: application/json' \
  -d '{
    "title": "Morning review",
    "request": "Review recent tasks and continue useful work.",
    "cron": "0 9 * * 1-5",
    "timezone": "Asia/Tokyo",
    "enabled": true
  }'
```

Schedules are stored in `~/.commander/schedules/crons.json`.
Use `POST /v1/schedules/{id}/run` to run one manually.

### Watch

Watch runs Commander on a schedule using instructions from `WATCH.md` in the workspace root.
Create `~/.commander/config/watch.json` to enable it:

```json
{
  "enabled": true,
  "schedules": [
    {
      "cron": "*/20 * * * *",
      "timezone": "Asia/Tokyo"
    }
  ]
}
```

If `watch.json` is missing or `enabled` is `false`, Watch does not run.
The example above runs every 20 minutes.

## Development

Run tests:

```bash
cargo test
```

Run lints:

```bash
cargo clippy
```
