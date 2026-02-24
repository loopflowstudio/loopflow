# Claude Adapter Plan (next)

Add Claude as a second session provider in `lfd`, using the existing normalized session schema (`Command`, `File`, `Message`, `Thought`, `Tool`).

## Goal

- Run Claude through `/v0/sessions` with the same lifecycle and replay semantics as Codex.
- Validate adapter abstraction against a non-JSON-RPC provider.

## Approach

Use process-per-turn invocation:

- `claude -p ... --output-format stream-json`
- First turn captures `system.session_id`
- Later turns use `--resume <session_id>`

Each `send_input()` spawns a Claude process and streams NDJSON events into `SessionEvent`.

## Key decisions

### 1) Process-per-turn (not persistent)

Claude `-p` exits at end of turn, so continuity comes from `--resume`. `start()` validates environment; `send_input()` does turn execution.

### 2) Config parity across providers

- `yolo_mode` → `--dangerously-skip-permissions`
- `max_turns` → `--max-turns`
- `system_prompt` → `--append-system-prompt` when present
- `model`, `cwd` pass through

### 3) Single in-flight turn per session

Reject concurrent `send_input()` with conflict/busy (`409`) to preserve deterministic ordering.

### 4) Durable provider session id

Persist Claude `system.session_id` to `sessions.provider_session_id` immediately so `--resume` remains durable.

## Event mapping

Claude NDJSON wraps API events under `stream_event.event`.

### Streaming deltas

- `content_block_delta` + `text_delta` → `TextDelta`
- `content_block_delta` + `thinking_delta` → `ReasoningDelta`

### Tool lifecycle

- `content_block_start` (`tool_use`) → `ItemStarted`
- `input_json_delta` chunks are accumulated in adapter state
- `user` tool result for matching `tool_use_id` → `ItemCompleted`

### Turn boundaries

- process start → `TurnStarted`
- `result` event → `TurnCompleted`
- process error/abnormal exit → `TurnCompleted(Failed)`

## Session item inference

Claude gives tool names, not typed item categories. Map as:

- `Bash` → `Command`
- `Edit`, `Write`, `NotebookEdit` → `File`
- everything else → `Tool`

## Out of scope

- Diff synthesis when Claude does not emit a turn diff
- Incremental streaming of tool stdout/stderr (Claude returns tool results after completion)
- Concerto UI work

## Done when

- `provider: "claude"` session can be created and reaches `active`
- Input spawns Claude process and events appear via SSE replay/live stream
- Second turn resumes with persisted provider session id
- Concurrent input while turn is active returns conflict/busy
- `DELETE /sessions/{id}` stops any in-flight Claude process
- `cargo test --all` and `cargo clippy -- -D warnings` pass
