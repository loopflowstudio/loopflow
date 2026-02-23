# Review: Unified Session API + Codex Adapter

## What was implemented

Session lifecycle management for `lfd` with five HTTP endpoints, persistent event replay, and a Codex `--app-server` adapter.

- `POST /v0/sessions` — create a session (returns `starting`, transitions to `active` async)
- `GET /v0/sessions/{id}` — get session state
- `POST /v0/sessions/{id}/input` — send user input (routes to `turn/start` or `turn/steer`)
- `GET /v0/sessions/{id}/events` — SSE stream: replay from store, then live tail via broadcast
- `DELETE /v0/sessions/{id}` — graceful shutdown (idempotent)

Storage layer: new `sessions` and `session_events` tables (migration `010_sessions.sql`), implemented for both SQLite and PostgreSQL.

Codex adapter: spawns `codex --app-server`, communicates via JSON-RPC over stdin/stdout, maps notifications to structured `SessionEvent` variants, auto-accepts approval requests.

## Key choices

**Turn+item event model over flat deltas.** `SessionEvent` is a serde-tagged enum with structured variants: turn boundaries (`turn_started`, `turn_completed`), item lifecycle (`item_started`, `item_updated`, `item_completed`), high-frequency streaming deltas (`text_delta`, `reasoning_delta`), and session-level signals (`status_changed`, `error`). Items are typed (`Command`, `FileChange`, `McpToolCall`, `AgentMessage`, `Plan`, `Tool`) with a generic `Tool` fallback. All events persist as JSON in `session_events` — the stream is the single source of truth.

**Async create with `starting` status.** Create returns immediately; adapter startup happens in a spawned task. Avoids blocking the HTTP handler on process spawn + initialize handshake (up to 15s timeout). Clients poll or subscribe to events to detect `active`.

**Broadcast + store for event delivery.** Live events go through `tokio::broadcast`; all events are also persisted to the store. SSE handler replays from store first, then switches to broadcast with seq-based dedup. This means clients reconnecting after a disconnect get full replay without gaps.

**At-most-one active session per wave run.** Enforced at create time via a store query for non-terminal sessions on the same `wave_run_id`. Prevents resource contention.

**Factory pattern for adapters.** `SessionAdapterFactory` trait allows tests to inject a `FakeAdapter` without process spawning. `DefaultSessionAdapterFactory` dispatches on provider string.

## How it fits together

```
HTTP routes ──→ SessionManager ──→ Store (persist)
                     │                    ↑
                     ├── SessionRuntime ──┘ (append events)
                     │       │
                     │       ├── broadcast::Sender (live SSE)
                     │       └── Mutex<dyn SessionAdapter>
                     │                │
                     └── spawn tasks ─┘
                          ├── event bridge (adapter→store+broadcast)
                          └── startup (adapter.start→set_status)
```

`SessionManager` owns lifecycle transitions. `SessionRuntime` holds the per-session adapter, broadcast channel, and sequence counter. Background tasks bridge adapter events to persistence and broadcast. The HTTP layer is a thin translation from JSON requests to manager calls and from persisted events to SSE.

## Risks and bottlenecks

- **Codex JSON-RPC payload shapes are inferred.** `map_notification` covers `item/agentMessage/delta`, `item/started`, `item/completed`, etc. based on documented patterns, but real codex traces should validate these mappings.
- **Process-local runtimes.** Active sessions live in a `HashMap<LfdId, Arc<SessionRuntime>>` — they don't survive daemon restarts. Sessions in `starting` or `active` at restart time become orphans in the store. A startup recovery pass (like the existing agent recovery) would fix this.
- **Broadcast lagged receivers skip.** If a live SSE client can't keep up, broadcast drops messages and the client skips them. The store has the full history, but in-stream backfill from store isn't implemented — clients would need to reconnect to get missed events.
- **Single adapter lock.** `Mutex<Box<dyn SessionAdapter>>` serializes all operations (start, send_input, stop) per session. Fine for the current codex adapter where operations are fast JSON-RPC writes, but could bottleneck if future adapters have slow operations.

## What's not included

- Additional providers beyond codex (claude, opencode).
- Resume/rehydration of active sessions after daemon restart.
- SSE backfill for lagged broadcast receivers (clients must reconnect).
- Wave orchestration integration beyond optional `wave_run_id` metadata.
