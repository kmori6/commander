# Architecture

## Layers

Commander follows clean architecture. Dependencies flow inward; the domain has no knowledge of infrastructure.

```mermaid
graph TD
    P["Presentation<br/>(src/presentation/)"]
    A["Application<br/>(src/application/)"]
    D["Domain<br/>(src/domain/)"]
    I["Infrastructure<br/>(src/infrastructure/)"]

    P --> A
    A --> D
    I --> D
```

| Layer          | Responsibility                                     |
| -------------- | -------------------------------------------------- |
| Presentation   | CLI I/O, argument parsing, progress display        |
| Application    | Usecase orchestration                              |
| Domain         | Models, port interfaces, business logic            |
| Infrastructure | External service adapters (LLM, DB, search, tools) |

## Component Overview

```mermaid
graph LR
    subgraph Presentation
        CLI["agent_cli / research_cli<br/>survey_cli / digest_cli"]
    end

    subgraph Application
        AU["AgentUsecase"]
        RU["ResearchUsecase"]
        SU["SurveyUsecase"]
        DU["DigestUsecase"]
    end

    subgraph Domain
        AS["AgentService"]
        CS["ContextService"]
        TE["ToolExecutor"]
        DR["DeepResearchService"]
        MS["MemoryIndexService"]
    end

    subgraph Infrastructure
        LLM["BedrockLlmProvider<br/>(AWS Bedrock)"]
        EMB["BedrockEmbeddingProvider<br/>(AWS Bedrock)"]
        DB["PostgreSQL Repositories"]
        Search["TavilySearchProvider"]
        Tools["Tools<br/>(file, shell, web, ocr, asr, memory…)"]
    end

    CLI --> AU
    CLI --> RU
    CLI --> SU
    CLI --> DU
    AU --> AS
    AU --> CS
    AS --> TE
    RU --> DR
    AS --> LLM
    CS --> LLM
    DR --> LLM
    DR --> Search
    MS --> EMB
    MS --> DB
    AU --> DB
    TE --> Tools
    Tools --> MS
```

## Domain

**Ports** (interfaces implemented by infrastructure):

- `LlmProvider` — inference (`response`, `response_with_tool`, `response_with_structure`)
- `EmbeddingProvider` — text embeddings for semantic memory search
- `SearchProvider` — web search
- `Tool` — name, spec, default policy, execution logic

**Repositories**: `ChatSession`, `ChatMessage`, `TokenUsage`, `ToolApproval`, `ToolExecutionRule`, `MemoryIndex`

**Services**:

- `AgentService` — LLM + tool loop, approval pause/resume
- `ContextService` — context window management and compaction
- `ToolExecutor` — tool lookup, policy resolution, execution
- `DeepResearchService` — iterative deep research (TTD-DR algorithm)
- `MemoryIndexService` — Markdown chunking, embedding, index rebuild, and semantic search

## External Dependencies

```mermaid
graph LR
    C["Commander"]
    C --> Bedrock["AWS Bedrock<br/>(LLM inference, embeddings)"]
    C --> Tavily["Tavily<br/>(web search)"]
    C --> PG["PostgreSQL<br/>(persistence)"]
    C --> MD["markitdown<br/>(binary → Markdown)"]
```

## Further Reading

- [process.md](process.md) — Agent loop and approval flow
- [sequence.md](sequence.md) — Detailed sequence diagrams
- [tools.md](tools.md) — Tool list and execution policy
- [database.md](database.md) — Database schema
- [context.md](context.md) — Context window management
- [memory.md](memory.md) — Long-term memory and journal indexing
- [deep-research.md](deep-research.md) — Deep research algorithm
