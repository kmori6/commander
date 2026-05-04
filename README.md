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

| Command                         | Description                    |
| ------------------------------- | ------------------------------ |
| `/new`                          | Start a new session            |
| `/approve`                      | Approve pending tool execution |
| `/deny`                         | Deny pending tool execution    |
| `/tools`                        | Show tool execution status     |
| `/tool <tool> <allow\|ask\|deny>` | Set a tool execution rule      |
| `/usage`                        | Show session token usage       |
| `/attach <files...>`            | Stage files to attach          |
| `/files`                        | Show staged files              |
| `/detach <index\|all>`          | Remove staged files            |
| `/exit`                         | Quit                           |

The chat CLI subscribes to `/v1/events`, posts messages to `/v1/sessions/{id}/messages`, and resolves approvals with `/v1/sessions/{id}/approvals`.

### Research

Deep research on a given query. Saves a report to `outputs/research/`.

```bash
commander research
```

### Survey

Read and summarize an academic paper from a PDF file or URL. Saves a report to `outputs/survey/`.

```bash
commander survey <path-or-url> [--output <path>]
```

### Digest

Curate daily papers and tech news into a digest. Saves to `outputs/digest/`.

```bash
commander digest [--date <YYYY-MM-DD>] [--output <path>]
```

## Development

Run tests:

```bash
cargo test
```

Run lints:

```bash
cargo clippy
```
