# 04: Hardening

Edge cases, reconnect durability, wave integration, and production readiness.

## What exists after this

Interactive sessions handle real-world failure modes gracefully. Reconnect is reliable. Concurrent clients don't corrupt state. Session end triggers wave advancement.

## What to address

- **SSE lagged receiver backfill**: live broadcast currently drops messages for slow receivers instead of backfilling from store. Discovered in phase 01 — needs in-stream store fallback.
- **Reconnect durability**: cursor-based event replay, handle gaps, stale connections
- **Concurrent clients**: multiple Concerto instances viewing same session, single input owner
- **Double-end**: idempotent end with first-win terminal status
- **Wave integration**: session end triggers existing continue/commit logic; wave run state guards
- **lfd restart**: persisted history readable, active session recovery best-effort (active runtimes are process-local, no restart rehydration yet)
- **Process crash recovery**: detect dead adapter process, transition to failed state
- **Provider auth interruption**: keep session alive when possible, emit error events

## Done when

- Reconnect replays events correctly from any cursor position
- Two Concerto clients can view the same session without corruption
- lfd restart preserves event history for ended sessions
- Dead adapter processes detected and marked failed within reasonable time
- Session end advances wave run when appropriate
