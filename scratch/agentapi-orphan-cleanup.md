# lfd restart / orphan cleanup

## Status

Implemented on this branch. `lfd` startup recovery now repairs orphaned session state and reaps orphaned OpenCode servers owned by dead `lfd` processes.

## Problem

`SessionRuntime` is in-memory only. After an `lfd` restart, sessions left in `starting`/`active` became zombies in the DB, and `opencode serve` child processes could continue running without an owning runtime.

## Final behavior

`SessionManager::recover_orphaned_sessions()` now performs one startup recovery pass and returns:

- `sessions_failed`
- `opencode_servers_reaped`
- `reap_errors`

Recovery does two things:

1. **Fail orphaned sessions in the DB**
   - Finds sessions in `starting` or `active`.
   - Appends `lfd_restarted_orphaned_session` and `status_changed -> failed` events.
   - Persists `status=failed` and `ended_at=now`.

2. **Reap orphaned OpenCode servers**
   - Uses `~/.lf/runtime/opencode-servers.json` runtime metadata.
   - Registers `{opencode_pid, owner_lfd_pid, created_at}` on OpenCode start.
   - Unregisters on normal harness stop.
   - On startup, reaps only entries whose owner `lfd` PID is dead **and** whose PID still looks like `opencode serve`.
   - Removes reaped/stale entries and keeps failures non-fatal.

`bin/lfd.rs` now logs all recovery totals (sessions recovered, OpenCode servers reaped, reap errors).

## Key decisions

- Restart truth beats continuity: orphaned `starting`/`active` sessions are always failed.
- Cleanup is ownership-based, not name-based: avoid broad `pkill` blast radius.
- Startup janitor is best-effort: cleanup errors are logged, never startup-blocking.
- No schema migration: runtime registry file was sufficient for this scope.

## Validation

- `cargo fmt --all -- --check`
- `cargo clippy -p loopflow --all-targets -- -D warnings`
- `cargo test -p loopflow recover_orphaned_sessions`
- `cargo test -p loopflow opencode_runtime`

Note: full `cargo test -p loopflow` in this environment still fails Docker-specific tests when `/var/run/docker.sock` is unavailable.

## Out of scope / remaining limits

- No attempt to reconnect/resume orphaned live sessions.
- No generalized orphan cleanup for non-OpenCode harnesses.
- PID ownership checks reduce blast radius but cannot eliminate rare PID-reuse edge cases.
