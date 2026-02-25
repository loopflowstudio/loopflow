# OpenCode Adapter

Third adapter validates the session API is truly provider-agnostic. Three transports (JSON-RPC stdio, NDJSON stdio, HTTP+SSE) all map to one canonical event model.

## Problem

We have two adapters (Claude: NDJSON/stdio, Codex: JSON-RPC/stdio). Both are subprocess-based with stdio communication. Adding a third adapter over HTTP+SSE proves the session API doesn't leak transport assumptions and works with a fundamentally different communication model — network I/O instead of pipes.

OpenCode exposes a headless HTTP server (`opencode serve`) with REST endpoints and SSE event streaming. This is the natural third adapter: different process model (long-lived HTTP server vs. per-turn CLI spawn vs. persistent JSON-RPC process), different I/O (HTTP+SSE vs. stdout pipes), same session semantics.

## Approach

### Architecture: HTTP client harness

`OpenCodeHarness` spawns `opencode serve --port $PORT`, then communicates exclusively over HTTP. Unlike Claude (per-turn process spawn) or Codex (stdin/stdout JSON-RPC), this adapter is a **reqwest HTTP client** talking to a local server.

**Process model:** One long-lived `opencode serve` process per session. The harness owns the child process for lifecycle management but never touches its stdio — all communication goes through HTTP.

**Event consumption:** SSE stream from `GET /event` provides real-time bus events. The harness subscribes once at startup and keeps the connection alive. Events are filtered by session ID and mapped to canonical `SessionEvent` variants.

**Turn model:** `POST /session/:id/message` sends user input. OpenCode processes it asynchronously. The harness detects turn boundaries from SSE events (`session.status` transitions: `idle` → `active` → `idle`).

### Files

| File | Purpose |
|------|---------|
| `harness/opencode.rs` | `OpenCodeHarness` struct, `Harness` trait impl |
| `harness/opencode_mapping.rs` | OpenCode bus events → `SessionEvent` mapping |
| `harness/mod.rs` | Register `OpenCode` variant in `HarnessKind` |

### Startup sequence

1. Pick an ephemeral port (bind to port 0, read assigned port — or use a random high port and retry)
2. `Command::new("opencode").args(["serve", "--port", &port.to_string()])` — spawn with stderr piped for logging, stdout/stdin null
3. Poll `GET /` or health endpoint with exponential backoff (max 15s timeout, matching Codex)
4. `POST /session` to create an OpenCode session — store returned session ID
5. Subscribe to `GET /event` SSE stream — spawn async task to consume and map events
6. Emit `SessionEvent::StatusChanged { status: Active }` is handled by SessionManager, not the harness

### Per-turn flow

1. `send_input(content)` → guard against concurrent turns (same `TurnInProgressGuard` pattern)
2. Seed task prompt on first turn (same pattern as Claude/Codex)
3. `POST /session/:id/message` with `{ parts: [{ type: "text", text: content }] }`
4. SSE reader task picks up events and maps them:
   - `session.status` with `active` → `SessionEvent::TurnStarted`
   - `message.part.updated` with `TextPart` → `SessionEvent::TextDelta`
   - `message.part.updated` with `ReasoningPart` → `SessionEvent::ReasoningDelta`
   - `message.part.updated` with `ToolPart` → `SessionEvent::ItemStarted` / `ItemCompleted`
   - `permission.asked` → auto-approve via `POST /session/:id/permissions/:pid` with `{ response: "always" }`
   - `session.status` with `idle` → `SessionEvent::TurnCompleted`
   - `session.diff` → `SessionEvent::DiffUpdated`
   - `session.error` → `SessionEvent::Error`
5. Turn completes when `session.status` returns to `idle`

### Event mapping detail

OpenCode's bus events use a `{ type, properties }` envelope. The SSE stream delivers these as `data: { ... }` lines.

| OpenCode event | SessionEvent | Notes |
|----------------|-------------|-------|
| `session.status` (idle→active) | `TurnStarted` | Generate turn_id on transition |
| `session.status` (active→idle) | `TurnCompleted` | Completed status |
| `session.status` (→error) | `TurnCompleted` | Failed status |
| `message.part.updated` (TextPart) | `TextDelta` | `delta` field for incremental, `text` for full |
| `message.part.updated` (ReasoningPart) | `ReasoningDelta` | Same delta logic |
| `message.part.updated` (ToolPart, state=running) | `ItemStarted` | Map tool name to SessionItem |
| `message.part.updated` (ToolPart, state=completed) | `ItemCompleted` | Include output |
| `message.part.updated` (ToolPart, state=failed) | `ItemCompleted` | Failed status |
| `permission.asked` | (internal) | Auto-approve, don't emit |
| `session.diff` | `DiffUpdated` | Pass through diff string |
| `session.error` | `Error` | Map to error code + message |
| `session.created` | (ignored) | We created it, already know |
| `message.updated` | (ignored) | Redundant with part events |
| `server.connected` | (ignored) | Connection health only |

### Tool → SessionItem mapping

OpenCode's `ToolPart` includes a tool name. Map to typed items where possible:

| OpenCode tool pattern | SessionItem | Heuristic |
|----------------------|-------------|-----------|
| Tool with `command` field | `Command` | Shell execution tools |
| Tool with `file`/`path` field | `File` | File edit tools |
| Everything else | `Tool` | Generic fallback |

This is deliberately coarse. OpenCode's tool naming is provider-dependent (varies by underlying model). The `Tool` fallback handles unknowns gracefully — the same approach Codex mapping uses for unrecognized item types.

### Shutdown sequence

1. `POST /session/:id/abort` — stop any in-progress work
2. `DELETE /session/:id` — clean up server-side state
3. Kill child process — same `child.start_kill()` + `child.wait()` pattern
4. Abort SSE reader task
5. Abort stderr logger task

### Permission auto-approval

OpenCode emits `permission.asked` events with a `requestID`. The harness auto-approves all permissions via `POST /session/:id/permissions/:requestID` with `{ response: "always" }`. This matches Codex's pattern of accepting all server requests and Claude's `--dangerously-skip-permissions` flag.

### Error handling

- **SSE connection drop:** Emit `SessionEvent::Error { code: "opencode_disconnected" }` (terminal error, same as `codex_disconnected`)
- **HTTP request failures:** Log and retry once for transient errors (5xx, timeouts). Emit error event on persistent failure
- **Process exit:** Detected by SSE stream closing. Emit disconnect error if not during shutdown
- **Health check timeout:** Return error from `start()`, triggering session failure in SessionManager

### Prompt seeding

Same pattern as Claude and Codex: on first `send_input()`, prepend `system_prompt` + `task_prompt` to the user content. OpenCode's `POST /session/:id/message` accepts a `system` field — use this for the system prompt instead of prepending, keeping the message cleaner.

```
First turn:
  system: config.system_prompt
  parts: [{ type: "text", text: config.task_prompt + "\n\n" + user_content }]

Subsequent turns:
  parts: [{ type: "text", text: user_content }]
```

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Use `opencode` CLI per-turn (like Claude) | Simpler process model, no HTTP client needed | OpenCode's CLI doesn't support NDJSON/streaming output. `opencode serve` is the documented programmatic interface. |
| Connect to existing OpenCode server | No process management needed | Requires user to run server separately. Other harnesses manage their own processes — consistency matters. |
| WebSocket instead of SSE | Bidirectional, could avoid separate HTTP calls | OpenCode doesn't offer WebSocket. SSE is what they provide. |
| Poll `/session/:id` for state changes | Avoid SSE complexity | Latency, bandwidth waste, misses streaming text deltas. SSE exists for this. |

## Key decisions

**Ephemeral port allocation.** Bind a TCP listener to port 0 to get an OS-assigned port, close it, then pass the port to `opencode serve`. Tiny race window but standard practice. Alternative: parse stdout for port — but we're not reading stdout.

**Single SSE connection, filter by session ID.** OpenCode's `/event` endpoint streams all events. We filter by `sessionID` in properties. One connection per harness instance is sufficient — we only create one OpenCode session per harness.

**`reqwest` for HTTP.** Already a dependency (`Cargo.toml:66`). No need to add a dedicated SSE client crate — parse the `text/event-stream` format manually. It's simple: `data: {json}\n\n` lines. A ~30-line parser in the reader task handles it.

**Turn boundaries from `session.status`.** OpenCode doesn't have explicit turn start/complete events like Codex. Instead, the session status transitions between `idle` and `active`. This is the turn signal. We generate `turn_id` on `idle→active` transition.

**Terminal error code: `opencode_disconnected`.** Add to `is_terminal_harness_error()` alongside `codex_disconnected` and `claude_harness_crashed`. This triggers the event bridge to mark the session as failed.

## Scope

**In scope:**
- `OpenCodeHarness` implementing `Harness` trait
- `opencode_mapping.rs` for event translation
- Registration in `HarnessKind` enum
- Unit tests for mapping functions
- Terminal error code registration

**Out of scope:**
- OpenCode-specific session config (model selection, etc.) — use generic `SessionConfig` fields
- OpenCode skill injection — unlike Claude, OpenCode manages its own tools
- Integration tests requiring a real `opencode` binary — unit test the mapping, smoke test the harness structure
- Session API changes — if the adapter needs API changes, stop and reconsider

## What earlier phases taught us

Phase 04 (hardening) established a conformance replay test pattern: record real provider traces, replay them through the harness, and assert canonical event output. Five traces exist today — Claude normal/crash/multi-tool, Codex normal/error — covering both happy paths and failure modes. The OpenCode adapter should follow this same pattern: record `opencode serve` SSE traces and build replay tests before wiring up the live adapter. This validates the event mapping independently of OpenCode availability.

Phase 04 also showed that SSE lag recovery (store-backed backfill on `Lagged`) is load-bearing, not optional. The OpenCode adapter's HTTP+SSE transport means events flow through the same bridge→broadcast→SSE path and will benefit from this recovery automatically.

## Done when

- `POST /sessions` with `harness: "opencode"` spawns OpenCode server and creates a session
- Events stream through `GET /sessions/{id}/events` as SSE with correct `SessionEvent` types
- `POST /sessions/{id}/input` sends a message, OpenCode responds, text deltas flow
- `DELETE /sessions/{id}` stops OpenCode, kills process, cleans up
- No session API changes required
- All three adapters pass conformance replay tests against recorded traces
- `is_terminal_harness_error("opencode_disconnected")` returns true
- Unit tests pass for `opencode_mapping` (text parts, tool parts, status transitions)
- `cargo clippy -- -D warnings` clean, `cargo fmt` clean
- All three adapters pass the same integration test pattern: create → input → events → end
