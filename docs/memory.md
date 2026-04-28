# Memory

Commander memory is an explicit, tool-driven store for durable facts and daily work notes.

## Files

Memory files live inside the current workspace:

| File                                      | Purpose                                                |
| ----------------------------------------- | ------------------------------------------------------ |
| `.commander/memory/MEMORY.md`             | Durable long-term facts, preferences, and stable notes |
| `.commander/memory/journal/YYYY-MM-DD.md` | Daily work notes, decisions, and session context       |

## Tools

| Tool            | Description                                                                 |
| --------------- | --------------------------------------------------------------------------- |
| `memory_write`  | Appends Markdown to long-term memory or a daily journal file                |
| `memory_search` | Searches indexed journal memory chunks using semantic similarity            |

`memory_write` accepts:

- `target`: `memory` or `journal`
- `content`: final Markdown text to append
- `journal_date`: optional `YYYY-MM-DD` date for journal entries; defaults to the local date

Long-term memory writes append to `.commander/memory/MEMORY.md` and skip indexing. Journal writes append to the selected daily file and rebuild the semantic index for that file.

`memory_search` accepts a non-empty `query` and an optional `limit` from 1 to 20. The default limit is 5. Results include the memory file path, chunk index, and matching content.

## Indexing Flow

Journal indexing is handled by `MemoryIndexService`:

1. Split Markdown into blank-line-separated blocks.
2. Pack blocks into chunks of up to 1024 characters, splitting long blocks when needed.
3. Generate one embedding per chunk with `EmbeddingProvider`.
4. Replace all `memory_index` rows for the journal path in PostgreSQL.
5. Search by embedding the query and ordering indexed chunks by pgvector distance.

If indexing fails after a journal write, the write still succeeds and the tool returns `index_status: "stale"` with the indexing error. Successful journal writes return `index_status: "indexed"`. Long-term memory writes return `index_status: "skipped"`.

## Persistence

The source of truth is the Markdown file under `.commander/memory/`. PostgreSQL stores only the searchable journal index in `memory_index`.

The current Bedrock embedding adapter uses Amazon Titan Text Embeddings V2 with 1024 normalized dimensions.
