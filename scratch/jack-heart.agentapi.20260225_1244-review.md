# OpenCode Adapter — Design Review

## What was implemented

Third session harness for `lfd`: `OpenCodeHarness` spawns `opencode serve` as a long-lived HTTP+SSE server process per session. Harness communication is REST (session create, message, permission approve, abort, delete) with SSE event subscription. OpenCode bus events are mapped into canonical `SessionEvent` values via `opencode_mapping.rs`. The adapter required no changes to the session API, event model, or client protocol — the harness abstraction held.

New files:
- `rust/loopflow/src/lfd/sessions/harness/opencode.rs` (571 lines) — harness lifecycle, SSE reader, HTTP transport
- `rust/loopflow/src/lfd/sessions/harness/opencode_mapping.rs` (652 lines) — event mapping, turn state machine, tool lifecycle deduplication

## Key choices

**HTTP+SSE instead of stdio.** OpenCode exposes `opencode serve` with REST endpoints and an SSE `/event` stream. Unlike Claude (NDJSON stdio) and Codex (JSON-RPC stdio), the harness acts as an HTTP client to a child process that runs its own server. This is the natural fit for OpenCode's architecture.

**Ephemeral port allocation.** Bind-to-0 then spawn. Known TOCTOU race window between port release and process bind, but this is the standard pattern and the window is small. Alternative (port passed via OpenCode config/env) would require OpenCode changes.

**SSE disconnect is terminal.** SSE stream loss fails the session immediately (`opencode_disconnected`). Reconnect logic is deferred to the hardening phase. This is consistent with how Claude and Codex treat process death — terminal, not recoverable (yet).

**Auto-approve permissions.** `permission.asked` events are automatically approved with `"response": "always"` to preserve non-interactive flow. This matches the container-first safety model: permissions come from the container boundary, not from per-tool approval.

**Defensive session-id parsing.** `parse_session_id` tries `id`, `sessionID`, `sessionId`, and nested `session.id`. This hedges against an inferred schema. The hardening plan explicitly calls for replacing these fallbacks with canonical names once recorded traces confirm the actual shape.

**Single-retry HTTP requests.** `send_request_with_retry` retries once on server errors (5xx) and transient network errors (timeout, connect). Enough to handle momentary hiccups without masking real failures.

## How it fits together

```
lfd SessionManager
  └── SessionRuntime
        └── OpenCodeHarness
              ├── spawns `opencode serve --port <ephemeral>`
              ├── waits for health check (exponential backoff, 15s deadline)
              ├── creates provider session via POST /session
              ├── SSE reader task (GET /event → map_event → mpsc → bridge → store)
              └── send_input: POST /session/{id}/message
```

The SSE reader task runs the full event loop: parse SSE frames, deserialize JSON, map via `opencode_mapping::map_event`, dispatch `SessionEvent`s through the unbounded mpsc channel, and auto-approve permission requests. Turn boundaries come from `session.status` transitions (`idle → active → idle`). Tool lifecycle events are deduplicated via `ToolLifecycle` (started/completed flags per tool id).

## Risks and bottlenecks

- **Inferred schema.** OpenCode event payloads are based on observation, not a published spec. Two open questions remain (see `scratch/questions.md`): `POST /session` response shape and `ToolPart` field names. The defensive multi-key fallbacks mask potential mismatches.
- **Port race.** Ephemeral port allocation has a small TOCTOU window. Unlikely to hit in practice but not zero.
- **No integration tests.** CI coverage is unit-level only. No real `opencode` binary in CI. Conformance replay tests (matching Claude/Codex pattern) are the next step.
- **SSE reconnect not implemented.** Transient SSE drops (TCP reset, sleep/wake) will fail the session. This is the same gap Claude and Codex have, but OpenCode is more exposed because SSE streams are more fragile than stdio pipes.

## What's not included

- **Harness reconnect / SSE resume.** Deferred to hardening phase.
- **Conformance replay tests.** Need recorded traces from a live OpenCode server.
- **Orphan cleanup.** `opencode serve` processes on lfd restart — tracked in hardening.
- **Schema pinning.** Defensive fallbacks remain until traces confirm canonical field names.

## Validation

- `cargo fmt --all -- --check` — clean
- `cargo clippy --all-targets -- -D warnings` — clean
- 51 session tests pass (including conformance replays for Claude/Codex)
- 31 OpenCode-specific tests pass (harness, mapping, engine integration)
- No regressions in existing harness behavior

## Wave alignment

This branch completes `wave/agentapi/01-opencode-adapter.md` (removed from wave). The hardening plan (`02-hardening.md`) is updated to reflect learnings and new OpenCode-specific concerns. The wave README risk section and architecture diagram now reflect all three harnesses.
