# 04: Hardening

Edge cases, reconnect durability, wave integration, and production readiness.

## What exists after this

Interactive sessions handle real-world failure modes gracefully. Reconnect is reliable. Concurrent clients don't corrupt state. Session end triggers wave advancement.

## What to address

- **SSE lagged receiver backfill**: `tokio::broadcast` with 256-entry buffer drops messages for slow receivers. Current behavior: skip and continue. Needs in-stream store fallback — detect `Lagged`, read missed events from store, resume broadcast. This is the highest-priority item since it affects basic reliability.
- **Reconnect durability**: `after_seq` cursor-based replay works for clean reconnects. Handle: stale SSE connections (keep-alive timeout detection), client reconnect after broadcast lag (seamless store backfill).
- **Concurrent clients**: multiple Concerto instances can subscribe to the same broadcast channel. Input routing is already single-adapter — no conflict. Need to verify broadcast fan-out works under load.
- **Double-end**: idempotent end is already implemented (first-win terminal status). Verify edge case: end during `starting` state.
- **Wave integration**: session end triggers existing continue/commit logic; wave run state guards
- **lfd restart**: `SessionRuntime` lives in a `HashMap` in memory — active sessions become orphans on restart. Events and session metadata survive in the store. Need a startup recovery pass to mark orphaned `active`/`starting` sessions as `failed`.
- **Process crash recovery**: detect dead adapter process (child process exit), transition to `failed` state, emit `Error` event. The Codex adapter's reader task already sees EOF on stdout and closes the event sender — the bridge task detects this and can transition state. Claude's process-per-turn model is different: normal exits happen after every turn, only non-zero exits during an active turn are crashes. The adapter already emits `TurnCompleted(Failed)` on abnormal exit, but the bridge task should also transition session status to `failed` if the error is unrecoverable.
- **Reader-task-stop race (Claude)**: `stop()` kills the child process and aborts the reader task. If the process exits normally between the kill and abort, a stale `TurnCompleted(Completed)` event may be emitted alongside the stop flow. The `AtomicBool` guard prevents state corruption but event ordering can be surprising. Consider draining the reader before emitting stop events.
- **Malformed tool input on crash**: Claude's `input_json_delta` chunks are concatenated and parsed at tool completion. If the process crashes mid-tool, accumulated partial JSON is silently dropped (`.ok()`). Should emit an `ItemCompleted` with `Failed` status for any in-flight tool when the process exits abnormally.
- **Provider auth interruption**: keep session alive when possible, emit error events

## Done when

- Reconnect replays events correctly from any cursor position
- Two Concerto clients can view the same session without corruption
- lfd restart preserves event history for ended sessions
- Dead adapter processes detected and marked failed within reasonable time
- Session end advances wave run when appropriate
