# Unified Session API + Codex Adapter

Standalone session management for `lfd` with durable event replay and a Codex app-server adapter.

## Current state

Implemented in this branch:

- Session lifecycle manager with `starting → active → ending → ended|failed`.
- `/v0/sessions` HTTP endpoints:
  - `POST /v0/sessions`
  - `GET /v0/sessions/{id}`
  - `POST /v0/sessions/{id}/input`
  - `GET /v0/sessions/{id}/events` (SSE replay + live follow)
  - `DELETE /v0/sessions/{id}`
- Storage migration `010_sessions.sql` with append-only `session_events`.
- Sqlite + Postgres store methods for sessions and session event persistence/replay.
- Codex adapter (`codex --app-server`) with JSON-RPC notification mapping.
- Manager tests for lifecycle behavior, provider gating, and one-active-session-per-wave-run enforcement.
- API docs in `docs/lfd.md` with curl examples.

## Event model (revised after review)

Turns are first-class. Items are typed with lifecycles. High-frequency deltas stay as top-level events for streaming efficiency.

```rust
enum SessionEvent {
    // Turn boundaries
    TurnStarted { turn_id: String },
    TurnCompleted { turn_id: String, status: TurnStatus },

    // Item lifecycle
    ItemStarted { turn_id: String, item: SessionItem },
    ItemUpdated { turn_id: String, item_id: String, data: ItemDelta },
    ItemCompleted { turn_id: String, item: SessionItem },

    // High-frequency streaming (separate from items for performance)
    TextDelta { turn_id: String, content: String },
    ReasoningDelta { turn_id: String, content: String },

    // Turn-level aggregates
    DiffUpdated { turn_id: String, diff: String },

    // Session-level
    StatusChanged { status: SessionStatus },
    Error { code: String, message: String },
}

enum SessionItem {
    Command { id, command: Vec<String>, cwd, status: ItemStatus,
              output: Option<String>, exit_code: Option<i32> },
    FileChange { id, changes: Vec<FileEdit>, status: ItemStatus },
    McpToolCall { id, server, tool, status: ItemStatus,
                  arguments: Value, result: Option<String> },
    AgentMessage { id, text, phase: Option<String> },
    Plan { id, text },
    Tool { id, name, status: ItemStatus,
           input: Option<Value>, output: Option<String> },
}

enum ItemStatus { InProgress, Completed, Failed, Declined }

enum ItemDelta {
    Output { content: String },
    PlanText { content: String },
}
```

### Design rationale

- **Turn IDs everywhere**: every turn-scoped event carries `turn_id`. Clients group by turn without scanning for start/complete pairs.
- **Typed items**: Concerto can render commands, file changes, and MCP calls differently. Generic `Tool` fallback for providers that don't distinguish.
- **Item lifecycle**: `ItemStarted → ItemUpdated* → ItemCompleted` mirrors Codex's `item/started → deltas → item/completed`. Item status captures failed/declined (not just done).
- **Streaming deltas stay top-level**: `TextDelta` and `ReasoningDelta` are high-frequency and don't need item wrapping. They carry `turn_id` for grouping.
- **No items table**: items are events in `session_events` with richer payloads. The event stream is the single source of truth.

### Codex mapping

| Codex notification | SessionEvent |
|---|---|
| `turn/started` | `TurnStarted { turn_id }` |
| `turn/completed` | `TurnCompleted { turn_id, status }` |
| `item/started` (commandExecution) | `ItemStarted { turn_id, Command { .. } }` |
| `item/started` (fileChange) | `ItemStarted { turn_id, FileChange { .. } }` |
| `item/started` (mcpToolCall) | `ItemStarted { turn_id, McpToolCall { .. } }` |
| `item/started` (plan) | `ItemStarted { turn_id, Plan { .. } }` |
| `item/completed` | `ItemCompleted { turn_id, item }` |
| `item/agentMessage/delta` | `TextDelta { turn_id, content }` |
| `item/reasoning/summaryTextDelta` | `ReasoningDelta { turn_id, content }` |
| `item/commandExecution/outputDelta` | `ItemUpdated { turn_id, item_id, Output { content } }` |
| `item/fileChange/outputDelta` | `ItemUpdated { turn_id, item_id, Output { content } }` |
| `item/plan/delta` | `ItemUpdated { turn_id, item_id, PlanText { content } }` |
| `turn/diff/updated` | `DiffUpdated { turn_id, diff }` |

### Claude mapping (phase 02)

| Claude event | SessionEvent |
|---|---|
| `content_block_delta` (text) | `TextDelta { turn_id, content }` |
| `content_block_delta` (thinking) | `ReasoningDelta { turn_id, content }` |
| `content_block_start` (tool_use, name=bash) | `ItemStarted { turn_id, Command { .. } }` |
| `content_block_start` (tool_use, name=edit) | `ItemStarted { turn_id, FileChange { .. } }` |
| `content_block_start` (tool_use, other) | `ItemStarted { turn_id, Tool { .. } }` |
| `content_block_stop` | `ItemCompleted { turn_id, item }` |
| `message_stop` | `TurnCompleted { turn_id, status }` |

## API behavior

### Session create

`POST /v0/sessions` returns immediately with `status: "starting"` and transitions to `active` asynchronously after adapter startup completes.

### Session input

`POST /v0/sessions/{id}/input` is valid only when the session is `active`. The codex adapter forwards input as `turn/start` or `turn/steer`. The manager assigns a `turn_id` and emits `TurnStarted` before forwarding.

### Session events

`GET /v0/sessions/{id}/events` replays persisted events from storage, then follows live events over SSE. Clients can pass `after_seq` to skip older replay items.

### Session end

`DELETE /v0/sessions/{id}` is idempotent and performs graceful shutdown (`turn/interrupt` when needed, then process stop).

## Core architecture

- `SessionManager` owns lifecycle transitions, persistence, and live broadcast.
- Replay path is store-backed (`session_events`); live tail is broadcast-backed.
- At most one active session per `wave_run_id`.
- Current provider scope is intentionally codex-only.

## Known risks and follow-ups

- Codex JSON-RPC payload assumptions were inferred and should be validated against real codex traces.
- Active runtimes are process-local; restart rehydration is not implemented.
- SSE lagged receivers currently skip missed live messages instead of in-stream store backfill.

## Out of scope in this iteration

- Wave orchestration beyond optional `wave_run_id` metadata.
- Additional providers beyond codex.
- Resume/rehydration of active adapter processes after daemon restart.
