# lfd restart / orphan cleanup

## Problem

`SessionRuntime` is in-memory only. After an `lfd` restart, sessions left in `starting`/`active` are zombies in the DB, and OpenCode child servers can keep running with no owning runtime.

Who benefits:
- Concerto users stop seeing permanently "active" sessions that cannot be resumed.
- Operators avoid leaked `opencode serve` processes and port drift after daemon crashes/restarts.
- Session API consumers regain a truthful lifecycle signal (`failed` instead of stuck non-terminal state).

Why now:
- Session status recovery exists conceptually in the hardening plan, but OpenCode process cleanup is still missing.
- Restart reliability is a core Agent API risk called out in `wave/agentapi/README.md`.

## Approach

Implement a single startup recovery pass in `SessionManager::recover_orphaned_sessions()` with two outputs: fail orphaned sessions and reap orphaned OpenCode servers.

1. **Fail orphaned sessions (authoritative DB repair)**
   - Query sessions in `starting` or `active`.
   - Append an error event (`lfd_restarted_orphaned_session`) and a `status_changed -> failed` event.
   - Persist session status `failed` with `ended_at=now`.

2. **Track OpenCode server ownership (during normal runtime)**
   - Add a small OpenCode runtime registry file under `~/.lf/runtime/opencode-servers.json`.
   - Record entries on successful OpenCode startup: `{opencode_pid, owner_lfd_pid, created_at}`.
   - Remove entries on normal `Harness::stop()`.

3. **Reap orphaned OpenCode servers at startup (best-effort janitor)**
   - During `recover_orphaned_sessions`, scan registry entries.
   - For each entry where `owner_lfd_pid` is dead, verify `opencode_pid` still looks like `opencode serve`, then `SIGTERM` it.
   - Remove reaped/stale entries from the registry.
   - Keep cleanup idempotent and non-fatal (log warnings; never block daemon startup).

4. **Expose recovery totals in logs**
   - Return a `SessionStartupRecovery` summary (`sessions_failed`, `opencode_servers_reaped`, `reap_errors`) and log it from `bin/lfd.rs`.

This is the same "managed resource janitor" pattern already used for Docker container orphan cleanup: explicit ownership metadata + startup sweep.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| `pkill -f "opencode serve"` on every startup | Very simple, catches most leaks | Too much blast radius; can kill user-managed OpenCode servers unrelated to `lfd` |
| Persist OpenCode PID in `sessions` table via schema migration | Strong DB audit trail, no sidecar file | Higher migration and plumbing cost (new columns + write paths) for a narrow cleanup need |
| **Registry file with owner `lfd` PID (chosen)** | Extra local state file to maintain | Chosen: precise enough, no DB migration, supports multi-`lfd` safety by skipping entries whose owner PID is still alive |

## Key decisions

- **Decide that restart truth beats continuity.** Orphaned `starting`/`active` sessions are always failed on boot; no attempt to "resume" unknown runtime state.
- **Decide cleanup must be ownership-based, not name-based.** We only kill OpenCode processes that a dead `lfd` instance owned.
- **Decide cleanup is best-effort and startup-safe.** Reap failures are logged, not fatal.
- **Wild success target:** users restart `lfd` after a crash and immediately see honest failed sessions, zero leaked OpenCode servers, and can start fresh sessions without manual cleanup.
- **Wild failure to avoid:** naive process matching kills unrelated OpenCode usage; stale PID reuse kills the wrong process; startup recovery blocks daemon boot.

## Scope

- In scope:
  - Startup failover of orphaned `starting`/`active` sessions.
  - OpenCode runtime registry (register/unregister/reap).
  - Startup logging/reporting for session + OpenCode recovery.
  - Unit/integration tests for recovery behavior and idempotency.
- Out of scope:
  - Reconnecting and resuming a live orphaned session after restart.
  - General process janitor for non-OpenCode harnesses.
  - New UI affordances in Concerto.

## Done when

- `cargo test -p loopflow recover_orphaned_sessions` passes (existing + new recovery tests).
- Manual crash-restart check:
  1. Start an OpenCode session.
  2. Kill `lfd` abruptly.
  3. Restart `lfd`.
  4. Observe session status becomes `failed` and orphaned `opencode serve` process is gone (`pgrep -fal "opencode serve"` shows no stale daemon from the dead `lfd`).
