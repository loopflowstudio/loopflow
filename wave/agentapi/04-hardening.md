# 04: Hardening

Edge cases, reconnect durability, wave integration, and production readiness.

## What exists after this

Interactive sessions handle real-world failure modes gracefully. Reconnect is reliable. Concurrent clients don't corrupt state. Session end triggers wave advancement.

## What Phase 02 hardening already addressed

- **Busy turn rejection**: concurrent `send_input` during an active turn returns 409 without failing the session. `SessionManager` distinguishes `TurnAlreadyInProgress` from real harness errors. No further work needed.
- **Claude NDJSON parsing robustness**: turn parsing hardened against edge cases in `content_block_delta` handling. Tool state tracking simplified.

## What to address

- **SSE lagged receiver backfill**: `tokio::broadcast` with 256-entry buffer drops messages for slow receivers. Current behavior: skip and continue. Needs in-stream store fallback — detect `Lagged`, read missed events from store, resume broadcast. This is the highest-priority item since it affects basic reliability.
- **Reconnect durability**: `after_seq` cursor-based replay works for clean reconnects. Handle: stale SSE connections (keep-alive timeout detection), client reconnect after broadcast lag (seamless store backfill).
- **Concurrent clients**: multiple Concerto instances can subscribe to the same broadcast channel. Input routing is already single-adapter — no conflict. Need to verify broadcast fan-out works under load.
- **Double-end**: idempotent end is already implemented (first-win terminal status). Verify edge case: end during `starting` state.
- **Wave integration**: session end triggers existing continue/commit logic; wave run state guards
- **lfd restart**: `SessionRuntime` lives in a `HashMap` in memory — active sessions become orphans on restart. Events and session metadata survive in the store. Need a startup recovery pass to mark orphaned `active`/`starting` sessions as `failed`.
- **Process crash recovery**: detect dead harness process (child process exit), transition to `failed` state, emit `Error` event. The Codex harness's reader task already sees EOF on stdout and closes the event sender — the bridge task detects this and can transition state. Claude's process-per-turn model is different: normal exits happen after every turn, only non-zero exits during an active turn are crashes. The harness already emits `TurnCompleted(Failed)` on abnormal exit, but the bridge task should also transition session status to `failed` if the error is unrecoverable.
- **Reader-task-stop race (Claude)**: `stop()` kills the child process and aborts the reader task. If the process exits normally between the kill and abort, a stale `TurnCompleted(Completed)` event may be emitted alongside the stop flow. The `AtomicBool` guard prevents state corruption but event ordering can be surprising. Consider draining the reader before emitting stop events.
- **Malformed tool input on crash**: Claude's `input_json_delta` chunks are concatenated and parsed at tool completion. If the process crashes mid-tool, accumulated partial JSON is silently dropped (`.ok()`). Should emit an `ItemCompleted` with `Failed` status for any in-flight tool when the process exits abnormally.
- **Provider conformance tests**: Phase 02 hardening revealed NDJSON edge cases that unit tests didn't catch initially. Add integration-style tests that replay real provider traces (both Codex JSON-RPC and Claude NDJSON) through the harnesses.
- **Provider-layer unification prep**: leave clear seams so a later phase can reuse this same provider layer for both `lf` CLI runs and Session HTTP without rewriting event mapping twice.
- **Provider auth interruption**: keep session alive when possible, emit error events

## Done when

- Reconnect replays events correctly from any cursor position
- Two Concerto clients can view the same session without corruption
- lfd restart preserves event history for ended sessions
- Dead harness processes detected and marked failed within reasonable time
- Session end advances wave run when appropriate
