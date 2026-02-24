# 06: OpenCode Adapter

Third adapter validates the session API is truly provider-agnostic.

## What exists after this

OpenCode sessions work through the same session API. Three adapters with three different transports (JSON-RPC stdio, NDJSON stdio, HTTP+SSE) all map cleanly to the canonical event model.

## What Phase 04 set up

**Crash handling is provider-generic.** Shared `InFlightItems` tracking in `harness/common.rs` handles abnormal exit for Codex and Claude. OpenCode gets this for free — implement the drain-on-crash pattern and in-flight items are completed as failed with crash metadata.

**Replay and lifecycle are proven.** SSE replay with `session.replay_completed` sentinel, orphan session recovery, and idempotent end all work across Codex and Claude. OpenCode should require no changes to the session manager or event bridge.

**Hermetic test infrastructure exists.** `scripts/lib/lfd_runtime.py` + `scripts/lib/api_harness.py` provide reusable live-HTTP assertions. OpenCode smoke tests can follow the same pattern as the existing API smoke suite.

## What to build

### OpenCode adapter

Spawn `opencode serve --port $PORT`, communicate via HTTP.

**Startup:**
1. Spawn `opencode serve --port $PORT`
2. Wait for health check
3. `POST /session` to create session

**Per turn:**
1. `POST /session/:id/message` with user input
2. Subscribe to SSE events from OpenCode
3. Map to SessionEvent (text deltas, tool events, turn boundaries)
4. Auto-respond to all permission requests (`POST /session/:id/permissions/:pid`)

**Shutdown:**
1. `POST /session/:id/abort`
2. `DELETE /session/:id`
3. Kill process

### Protocol validation

- Same five session API endpoints, same SSE event stream
- No API changes required to support the third adapter
- If changes are needed, they must work for Codex and Claude too

## Done when

- `POST /sessions` with `provider: "opencode"` spawns OpenCode server
- Events stream through `GET /sessions/{id}/events` as SSE
- `POST /sessions/{id}/input` sends a message, OpenCode responds
- `DELETE /sessions/{id}` stops the agent
- No session API changes required
- All three adapters pass the same integration test pattern: create → input → events → end
