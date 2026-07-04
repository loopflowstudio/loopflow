# HumanLayer `hld` daemon architecture

Repo: `github.com/humanlayer/humanlayer`, `hld/` (Go). **`hld` is the opposite of
what we're building:** one central daemon (single Unix socket
`~/.humanlayer/daemon.sock`, single SQLite `~/.humanlayer/daemon.db`) managing
*many* sessions. We build one server *per wave*, no central daemon. That
difference is the lens for every recommendation.

## 1. Process & runtime model
**One daemon, many sessions.** `daemon.Daemon` owns `config`, `listener` (Unix
socket), `rpcServer`, `httpServer`, `sessions SessionManager`, `approvals`,
`eventBus`, `store`, `permissionMonitor`.

**Sessions are child processes.** `session.Manager` holds
`activeProcesses map[string]ClaudeSession`. Launch via `claudecode-go`
(`client.Launch`) spawning the `claude` binary. Contract:
```go
type ClaudeSession interface {
    Interrupt() error; Kill() error; GetID() string
    Wait() (*claudecode.Result, error); GetEvents() <-chan claudecode.StreamEvent
}
```
Agent output = a **Go channel of stream events**. A per-session `monitorSession`
goroutine ranges over `GetEvents()`, writes each raw event to the store, parses
into a `ConversationEvent` row, updates token/status, publishes a bus event.
`Wait()` returning / channel closing = completion/crash.

Socket dir `0700`, socket file `0600` — security is purely FS perms, no auth.
HTTP server binds TCP, default `127.0.0.1:7777`, supports port `0` → OS assigns →
prints `HTTP_PORT=<n>` to stdout. Single-instance: dial socket on start; live →
`ErrDaemonAlreadyRunning`; dead → unlink + proceed. Crash recovery:
`markOrphanedSessionsAsFailed()` at startup marks any `running`/`waiting`/
`starting` session `failed`; does **not** re-attach orphaned processes — resume
is a fresh `claude --resume` with the stored `claude_session_id`.

## 2. Persistence / store
SQLite = source of truth; `activeProcesses` = only meaningful in-memory state.
- **`sessions`** — wide: `id`, `run_id`, `claude_session_id`,
  `parent_session_id`, `query`, `summary`, `title`, `model`, `working_dir`,
  `max_turns`, system-prompt fields, `allowed/disallowed_tools`, `status`,
  timestamps, cost/token cols, `auto_accept_edits`,
  `dangerously_skip_permissions(+expires/timeout)`, `archived`, proxy,
  `editor_state`.
- **`conversation_events`** — the log: `id AUTOINCREMENT`, `session_id`,
  `claude_session_id`, `sequence`, `event_type`, `role`, `content`, `tool_id`,
  `tool_name`, `tool_input_json`, `parent_tool_use_id`, `tool_result_for_id`,
  `tool_result_content`, `is_completed`, `approval_status`, `approval_id`.
  Ordered by `sequence`.
- **`approvals`** — `status CHECK IN ('pending','approved','denied')`.
- **`raw_events`** (untouched JSON), `file_snapshots`, `mcp_servers`,
  `user_settings` (singleton), `schema_version`.
- **Migrations:** 22+ hand-rolled sequential ALTERs, incl. a "healing migration"
  fixing prior reorder inconsistencies — a scar of the append-only ALTER approach.
- **`GetSessionConversation()`** walks the `parent_session_id` chain to gather all
  `claude_session_id`s, orders events by `CASE` on session position + `sequence`
  to reconstruct chronological order across resumed/forked sessions. This
  complexity exists because one logical conversation spans many `claude` child
  processes (each resume = new claude session id).

## 3. Event bus
`hld/bus/` — plain in-process pub/sub. Envelope:
```go
type Event struct { Type EventType; Timestamp time.Time; Data map[string]interface{} }
type EventFilter struct { Types []EventType; SessionID string; RunID string }
```
Types: `new_approval`, `approval_resolved`, `session_status_changed`,
`conversation_updated`, `session_settings_changed`. `Subscribe` returns a
buffered `chan Event, 100`. `Publish` iterates subscribers, applies filter,
**non-blocking send → DROP on full**. **Backpressure = drop-on-full.** The bus is
*not* durable — the store is.

## 4. API layering — REST *and* JSON-RPC, one brain
- **JSON-RPC 2.0 over Unix socket** (`hld/rpc/`): line-delimited JSON,
  `handlers[method]`; `Subscribe` hijacks the raw conn for streaming; 30s
  heartbeat.
- **HTTP REST + OpenAPI (Gin) + SSE** (`hld/api/`): `/api/v1`, SSE at
  `GET /api/v1/stream/events`, Anthropic proxy passthrough, CORS `*`.
- Both inject the **same managers** — no second implementation. Both exist for
  **legacy** reasons (socket for TUI, HTTP/SSE for web UI). A migration artifact.
- SSE handler: `eventBus.Subscribe(ctx, filter)`; `data: %s\n\n`; 30s
  `: keepalive`; filter via query params; disconnect via `ctx.Done()`; flush per
  write.

## 5. Config
Daemon config small/flat: `SocketPath, DatabasePath, APIKey, APIBaseURL,
LogLevel, HTTPPort, HTTPHost, ClaudePath` (flags > env `HUMANLAYER_*` >
`humanlayer.json` > defaults). Per-launch `LaunchSessionConfig` embeds
`claudecode.SessionConfig` (query, model, workingDir, maxTurns, systemPrompt,
tools, mcpServers, permissionPromptTool) + `Title`, `AutoAcceptEdits`,
`DangerouslySkipPermissions(+Timeout)`, proxy. `ContinueSessionConfig`
re-specifies the overridable subset; unspecified inherited from parent row.

## What transfers to a per-wave server vs. central-daemon artifacts

**Adopt:**
1. **Agent output = channel of typed stream events → one supervisor that (a)
   appends to the durable log and (b) publishes to the bus.** Clean core; matches
   our `WaveRuntime` + supervisor. `ClaudeSession` (Interrupt/Kill/Wait/GetEvents)
   is a good minimal subagent-handle contract.
2. **Store = truth; bus = best-effort liveness (drop-on-full, non-blocking).**
   `GET /conversation` reads the log; SSE subscribes to the bus; client re-syncs
   from the log. Keep the bus dumb.
3. **`conversation_events` row shape** — steal near-verbatim (incl. tool-call/
   result linkage).
4. **SSE mechanics** — query-param filter, flush per frame, `: keepalive`,
   disconnect via request context.
5. **Port 0 → print actual port to stdout** — perfect for per-wave discovery
   (our `.wave-endpoint`). Arguably more relevant to us than them.
6. **Orphan reconciliation on startup.**

**Do NOT copy:**
1. Two protocols — ship HTTP+SSE only; no TUI-over-socket back-compat to keep.
2. The many-sessions manager / `activeProcesses` map — our server *is* one wave.
3. `parent_session_id` chain-walking for conversation assembly — only needed if a
   thread fragments across processes; prefer one continuous log (`ORDER BY
   sequence`).
4. FS-perms-only security tied to a well-known socket — bind `127.0.0.1` +
   per-wave token.
5. 22+ hand-rolled ALTER migrations + "healing" migration — start clean; a
   per-wave DB is young/disposable.
6. The wide denormalized `sessions` row — dense because they list millions of
   sessions cheaply; per wave most of it is singleton metadata or belongs
   elsewhere.

**Tension:** their bus carries only liveness (`session_status_changed`,
`conversation_updated`) — SSE clients get "something changed" and re-fetch; the
bus does **not** stream message content. Decide deliberately: SSE streams full
deltas (lower latency, bus load-bearing, drop-on-full hurts) vs
change-notifications + re-fetch from `/conversation` (their conservative,
resync-safe choice).

Files: `hld/daemon/daemon.go`, `hld/session/{manager,claudecode_wrapper,types}.go`,
`hld/store/sqlite.go`, `hld/bus/{events,types}.go`, `hld/rpc/*`, `hld/api/handlers/*`,
`hld/config/config.go`, `hld/PROTOCOL.md`.
