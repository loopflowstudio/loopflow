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

Turn+item model. Turns are first-class. Items are typed with lifecycles. High-frequency deltas stay top-level for streaming efficiency.

```rust
enum SessionEvent {
    // Turn boundaries
    TurnStarted { turn_id },
    TurnCompleted { turn_id, status: TurnStatus },

    // Item lifecycle (typed items)
    ItemStarted { turn_id, item: SessionItem },
    ItemUpdated { turn_id, item_id, data: ItemDelta },
    ItemCompleted { turn_id, item: SessionItem },

    // Streaming deltas (high-frequency, separate from items)
    TextDelta { turn_id, content },
    ReasoningDelta { turn_id, content },

    // Turn-level aggregates
    DiffUpdated { turn_id, diff },

    // Session-level
    StatusChanged { status: SessionStatus },
    Error { code, message },
}

enum SessionItem {
    Command { id, command, cwd, status, output, exit_code },
    File { id, path, status, diff },
    Message { id, text },
    Thought { id, text },
    Tool { id, name, status, input, output },
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
  - `turn/started` → `TurnStarted { turn_id }`
  - `turn/completed` → `TurnCompleted { turn_id, status }`
  - `item/started` (commandExecution) → `ItemStarted { turn_id, Command { .. } }`
  - `item/started` (fileChange) → `ItemStarted { turn_id, File { .. } }`
  - `item/started` (mcpToolCall) → `ItemStarted { turn_id, Tool { .. } }`
  - `item/started` (plan) → `ItemStarted { turn_id, Thought { .. } }`
  - `item/completed` → `ItemCompleted { turn_id, item }`
  - `item/agentMessage/delta` → `TextDelta { turn_id, content }`
  - `item/reasoning/summaryTextDelta` → `ReasoningDelta { turn_id, content }`
  - `item/commandExecution/outputDelta` → `ItemUpdated { turn_id, item_id, Output }`
  - `item/plan/delta` → `ItemUpdated { turn_id, item_id, PlanText }`
  - `turn/diff/updated` → `DiffUpdated { turn_id, diff }`
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

All planned pieces landed: session API (5 endpoints under `/v0/sessions`), turn+item event model, lifecycle state machine, Sqlite + Postgres storage, Codex adapter with JSON-RPC mapping, manager tests. API docs in `docs/lfd.md`.

**Event model evolved from plan.** Started with flat events (`TextDelta, ToolStarted, ToolDone`). Shipped with a richer turn+item model — typed items with lifecycles, `turn_id` on all turn-scoped events, and a generic `Tool` fallback. This is better for both the Claude adapter (structured tool mapping) and Concerto (typed item rendering).

**Harness trait is simpler than expected.** Three methods: `start()`, `send_input()`, `stop()`. Provider dispatch via `HarnessProvider` enum, no factory trait needed. The abstraction is clean enough that Phase 02 was straightforward to wire up.

**Event normalization is where complexity lives.** The Codex harness spent most of its effort normalizing field name aliases (`turn.id` / `turnId` / `id`), status values (`cancelled` → `interrupted`), and item type detection from JSON-RPC payloads. Phase 02's Claude harness faced similar normalization work against NDJSON.

**What to validate before phase 02:** Codex event mappings were built from docs, not real traces. Run a real Codex session to confirm payloads match before using this as the reference for Claude harness event mapping.
