# Agent API Sessions: Current State and Next Work

This document is the source of truth for session API work in `lfd`.

## Current state

- `/v0/sessions` lifecycle endpoints are implemented: create/get/input/events/delete.
- Sessions persist to `sessions`; event streams persist append-only to `session_events`.
- SSE `/events` replays persisted history, then tails live events.
- `SessionManager` enforces lifecycle transitions (`starting → active → ending → ended|failed`).
- One active non-terminal session per `wave_run_id` is enforced.
- Normalized `SessionEvent` stream is in place:
  - Turn: `TurnStarted`, `TurnCompleted`
  - Item lifecycle: `ItemStarted`, `ItemUpdated`, `ItemCompleted`
  - Streaming: `TextDelta`, `ReasoningDelta`
  - Turn aggregate: `DiffUpdated` (provider-dependent)
  - Session: `StatusChanged`, `Error`
- Normalized `SessionItem` variants are used by clients: `Command`, `File`, `Message`, `Thought`, `Tool`.

## Provider support

### Codex

- `codex --app-server` harness is wired through the shared harness abstraction.
- Codex-specific JSON-RPC payloads are mapped into normalized session events.

### Claude

Claude is now supported as a second provider using process-per-turn execution:

- Invocation: `claude -p ... --output-format stream-json`
- Continuation: `--resume <provider_session_id>` on subsequent turns
- Provider session id durability: Claude `system.session_id` is persisted immediately to `sessions.provider_session_id`
- Concurrency: single in-flight turn per session (`409 busy` on concurrent input)
- Event parsing: supports both wrapped (`stream_event.event`) and top-level NDJSON events
- Error handling: `result.is_error` maps to `TurnCompleted(Failed)`

Tool/item mapping for Claude:

- `Bash` → `Command`
- `Edit` / `Write` / `NotebookEdit` → `File`
- everything else → `Tool`

## Claude integration notes

- Harness events flow over `broadcast::Sender<SessionEvent>` and are bridged into persistence + SSE broadcast by `SessionManager`.
- `ProviderSessionId` is treated as an internal event: persisted for resume, not emitted to SSE clients.
- Setup failures release the in-flight turn guard to avoid stuck busy sessions.

## Remaining gaps

1. **No restart rehydration**
   - Active runtimes are process-local; daemon restart loses live harness processes.
2. **SSE lag handling is reconnect-based**
   - Lagged subscribers can miss broadcast events and must reconnect for replay.
3. **No Claude turn diff synthesis**
   - Claude does not emit turn-level diffs; `DiffUpdated` is generally absent for Claude sessions.
4. **No incremental Claude tool output streaming**
   - Tool output arrives at completion, not as streaming stdout/stderr.
5. **Codex mapping still needs trace validation**
   - Keep validating inferred payload mapping against real traces.

## Next milestones

1. Concerto UI integration against session API streams.
2. Hardening pass:
   - restart recovery/rehydration
   - optional in-stream backfill for lagged SSE subscribers
   - stronger provider conformance tests (Codex + Claude)
3. OpenCode harness implementation on the normalized schema.
4. Provider layer convergence (planned):
   - make harness + mapping the shared core for both `lf` CLI execution and Session HTTP
   - keep CLI and HTTP as two API surfaces over one provider layer
   - add cross-surface conformance tests to prevent drift
