# 01: Unified Session API + Codex Adapter

Build the session API, event model, storage, and SSE replay. Codex as first adapter to prove the design.

## What exists after this

lfd exposes a session API. Clients create a Codex session, send input, receive structured events via SSE, and end the session. Events persist and replay on reconnect. The API, storage, and event model are provider-agnostic — Codex is just the first adapter.

## Why Codex first

Codex app-server has the lowest impedance — JSON-RPC over stdio with structured events for turns, items, and approvals. Building against it means the event model is informed by real structured events, not guesswork.

## What to build

### Session API

Five HTTP endpoints:

- `POST /sessions` — create session, launch adapter
- `GET /sessions/{id}` — current status + metadata
- `POST /sessions/{id}/input` — send user message
- `GET /sessions/{id}/events` — SSE replay (from seq 0) + follow (live tail)
- `DELETE /sessions/{id}` — graceful stop

### Event model

Flat typed events. No nesting (items within turns). Adapters emit what they can.

```rust
enum SessionEvent {
    TextDelta { content: String },
    TextDone { content: String },
    ToolStarted { tool_id: String, name: String, input: Option<Value> },
    ToolOutput { tool_id: String, content: String },
    ToolDone { tool_id: String },
    TurnStarted,
    TurnCompleted { status: TurnStatus },
    SessionStatus { status: SessionStatus },
    Error { code: String, message: String },
}
```

### Session lifecycle manager

- State machine: `starting → active → ending → ended | failed`
- Spawn adapter, wire event sink, track status transitions
- Idempotent end (multiple calls safe)
- At most one active session per wave run

### Storage

```sql
CREATE TABLE sessions (
    id TEXT PRIMARY KEY,
    provider TEXT NOT NULL,
    status INTEGER NOT NULL,
    wave_run_id TEXT,
    provider_session_id TEXT,
    config TEXT,
    created_at INTEGER NOT NULL,
    ended_at INTEGER
);

CREATE TABLE session_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id TEXT NOT NULL REFERENCES sessions(id),
    seq INTEGER NOT NULL,
    event_type TEXT NOT NULL,
    data TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    UNIQUE(session_id, seq)
);
```

### Codex adapter

- Spawn `codex --app-server` (JSON-RPC over stdio)
- Initialize: send `initialize` request, await `initialized` notification
- Create thread: `thread/start` with model and cwd
- Per turn: `turn/start` with user input, stream notifications
- Map Codex events to SessionEvent:
  - `item/agentMessage/delta` → `TextDelta`
  - `turn/started` → `TurnStarted`
  - `turn/completed` → `TurnCompleted`
  - `item/started` (commandExecution, fileChange, mcpToolCall) → `ToolStarted`
  - `item/commandExecution/outputDelta`, `item/fileChange/outputDelta` → `ToolOutput`
  - `item/completed` → `ToolDone`
- Auto-approve all approval requests (respond `accept` to JSON-RPC requests)
- Shutdown: `turn/interrupt`, kill process

## Probes (run early)

1. Codex app-server auto-approve: confirm responding `accept` to all approval JSON-RPC requests works cleanly
2. SSE replay + follow: is replaying from seq 0 then tailing sufficient, or do we need cursor-based pagination?

## Open questions

- Does Codex app-server support session resume after process restart?
- What happens to in-flight turns when we send interrupt?
- Codex JSON-RPC payload mappings were inferred from docs, not validated against real traces — run a real session before building the next adapter

## Shipped

All planned pieces landed: session API (5 endpoints under `/v0/sessions`), flat event model, lifecycle state machine, Sqlite + Postgres storage, Codex adapter with JSON-RPC mapping, manager tests. API docs in `docs/lfd.md`.

**What went as expected:** Event model, lifecycle state machine, storage schema, and API shape all landed as designed. The adapter trait abstraction worked cleanly.

**What to validate before phase 02:** Codex event mappings were built from docs, not real traces. Run a real Codex session to confirm payloads match before using this as the reference for Claude adapter event mapping.
