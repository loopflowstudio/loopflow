# 04: Hardening

Edge cases, reconnect durability, and production readiness for the agent runtime.

## What exists after this

Interactive agents handle real-world failure modes gracefully. Reconnect is reliable. Concurrent clients don't corrupt state. Provider auth interruptions don't kill sessions.

## What to address

- **Reconnect durability**: cursor-based event replay, handle gaps, stale connections
- **Concurrent clients**: multiple Concerto instances viewing same agent, single input owner
- **Double-end**: idempotent end with first-win terminal status
- **Provider auth interruption**: `AuthStatus` events, keep session alive when possible
- **Wave state mismatch**: guard against end when wave run no longer waits on this agent
- **lfd restart**: persisted history readable, active agent recovery best-effort
- **Process crash recovery**: detect dead adapter process, transition to failed state

## Done when

- Reconnect replays events correctly from any cursor position
- Two Concerto clients can view the same agent without corruption
- Auth interruptions emit events instead of killing sessions
- lfd restart preserves event history for ended agents
- Dead adapter processes detected and marked failed within reasonable time
