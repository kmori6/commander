# Tools

This document gives a short overview of the tools currently registered in `commander`.
The current source of truth is [src/main.rs](../src/main.rs).

## Workspace tools

- `file_search`: Find files in the workspace by glob-style path pattern.
- `file_read`: Read a file from the workspace. Text files are returned directly, and supported binary files such as PDF or Office documents are converted to Markdown. Optional line-range arguments can narrow the returned slice.
- `text_search`: Search text across workspace files.
- `file_write`: Write full UTF-8 text content to a file in the workspace. Parent directories are created automatically, and existing files are replaced.
- `file_edit`: Replace exactly one matched text block in a UTF-8 file in the workspace. Use `file_write` for full rewrites.
- `shell_exec`: Run one non-interactive shell command in the workspace, with optional `workdir`.

## Web and research tools

- `web_search`: Search the public web.
- `web_fetch`: Fetch and extract content from a web page.
- `research`: Higher-level web research workflow built on top of LLM-assisted search/fetch.

## Multimodal tools

- `asr`: Convert speech/audio input into text.
- `ocr`: Extract text from local images or PDF files.

## Notes

- The current workspace editing surface is intentionally split into small responsibilities: `file_write` handles full rewrites, `file_edit` handles exact one-shot replacements, and `shell_exec` stays a separate fallback for command execution.
- The current tool surface is strongest for workspace inspection, text operations, lightweight web access, and basic multimodal extraction.
- `shell_exec` is available now, but it should still be hardened further for safer command policy and execution boundaries.
- `research` is useful today, but it is closer to a workflow/skill than a low-level primitive tool.

## Execution policy and approval

Each tool can declare a default `ToolExecutionPolicy`:

- `Auto`
  - The tool may run automatically.
- `Ask`
  - The agent should pause and ask the user before executing the tool.
- `ConfirmEveryTime`
  - The agent should ask every time. This is a hard safety floor and should
    not be bypassed by a stored `allow` rule.

The current defaults are:

- Read/search/extraction tools default to `Auto`.
- `shell_exec` overrides the default to `Ask`.
- `file_write` overrides the default to `Ask`.
- `file_edit` overrides the default to `Ask`.

Persisted tool execution rules are stored in `tool_execution_rules`:

- `allow`
- `ask`
- `deny`

At runtime, Commander combines the tool's default policy with the persisted rule
to produce a final decision:

- `Allow`
  - Execute the tool.
- `Ask`
  - Pause the loop and create a pending approval request.
- `Deny`
  - Do not execute the tool. Return an error tool result to the LLM.

Unknown tool calls are treated like blocked calls and returned to the LLM as
error tool results.

## Approval commands

When a tool requires confirmation, the CLI shows the pending tool call and asks
the user to run one of these commands:

- `/approve`
  - Records an approval decision, reloads current tool execution rules, rechecks
    the pending tool, then resumes the agent loop.
- `/deny`
  - Records a denial decision, saves already accumulated tool results, marks the
    pending tool as denied, marks remaining deferred tools as skipped, and ends
    the paused turn with a denial message.

Pending approval state is currently in memory. Approval decisions are persisted
as logs, but pending approval recovery after process restart is not implemented
yet.
