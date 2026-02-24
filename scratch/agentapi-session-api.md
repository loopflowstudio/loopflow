# Agent API Sessions: Baseline and Next Work

This document is the current source of truth for session-api work in `lfd`.

## Baseline (already in branch)

- `/v0/sessions` lifecycle endpoints exist: create/get/input/events/delete.
- Sessions persist to `sessions`; event stream persists append-only to `session_events`.
- SSE `/events` replays persisted history, then tails live broadcast events.
- `SessionManager` enforces lifecycle transitions (`starting → active → ending → ended|failed`).
- Codex adapter (`codex --app-server`) is wired through the adapter abstraction.
- One active/non-terminal session per `wave_run_id` is enforced.
- Session item schema is normalized across providers:
  - `FileChange` → `File`
  - `AgentMessage` → `Message`
  - `Plan` → `Thought`
  - MCP calls map to generic `Tool`

## Event model (normalized)

`SessionEvent` keeps turn boundaries, item lifecycles, streaming deltas, and session-level signals in one replayable stream.

- Turn: `TurnStarted`, `TurnCompleted`
- Item lifecycle: `ItemStarted`, `ItemUpdated`, `ItemCompleted`
- Streaming: `TextDelta`, `ReasoningDelta`
- Turn aggregate: `DiffUpdated` (provider-dependent)
- Session: `StatusChanged`, `Error`

`SessionItem` variants used by clients:

- `Command`
- `File`
- `Message`
- `Thought`
- `Tool`

## Known gaps

1. **No restart rehydration**
   - Active runtimes are process-local; daemon restart leaves persisted sessions without live adapter processes.
2. **SSE lag handling is reconnect-based**
   - Lagged receivers can miss broadcast events; clients must reconnect to replay from store.
3. **Provider expansion still pending**
   - Claude and OpenCode adapters are not yet implemented in this branch.
4. **Codex mapping assumptions need ongoing validation**
   - JSON-RPC payload handling is inferred and should keep being validated against real traces.

## Next milestones

1. Claude adapter on top of normalized schema (`scratch/agentapi-claude-adapter.md`).
2. Concerto UI integration against the session API event stream.
3. Hardening pass:
   - restart recovery/rehydration
   - optional in-stream backfill for lagged SSE subscribers
   - stronger provider conformance tests
