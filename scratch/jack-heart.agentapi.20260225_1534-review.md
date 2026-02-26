# Review: lfd restart / orphan cleanup

## What was implemented

Startup recovery for `lfd` now handles two categories of orphans:

1. **Session state** — sessions stuck in `starting` or `active` after a crash are marked `failed` with proper event history (`lfd_restarted_orphaned_session` + `status_changed`).

2. **OpenCode server processes** — a runtime registry (`~/.lf/runtime/opencode-servers.json`) tracks `{opencode_pid, owner_lfd_pid}`. On startup, entries whose owner PID is dead are reaped if the process still looks like `opencode serve`. The harness registers on spawn and unregisters on clean stop.

`recover_orphaned_sessions()` now returns `SessionStartupRecovery` with `sessions_failed`, `opencode_servers_reaped`, and `reap_errors` instead of a bare count. `bin/lfd.rs` logs each metric separately.

## Key choices

- **Ownership-based reaping over name-based** — checking `owner_lfd_pid` liveness + process name avoids `pkill`-style blast radius. A stale entry whose PID was reused gets a second check (`ps -o command= -p PID` must contain "opencode" and "serve").
- **Runtime file, not DB** — the registry is a JSON file in `~/.lf/runtime/`, not a DB table. This avoids schema migration and keeps the concern scoped to process lifecycle, not persistent state.
- **Best-effort cleanup** — reap errors increment a counter and log a warning. They never block startup. If the registry file is corrupt or unreadable, one error is logged and startup proceeds.
- **Inject-for-test pattern** — `reap_orphaned_opencode_servers_at_path` takes closures for PID liveness, process matching, and termination. Tests exercise all reap logic without touching real processes.

## How it fits together

```
lfd startup
  ├─ recover_orphaned_sessions()
  │    ├─ list sessions in starting/active → fail each with events
  │    └─ reap_orphaned_opencode_servers()
  │         ├─ read registry file
  │         ├─ for each entry: owner dead? process matches? → kill + remove
  │         └─ write retained entries back
  └─ log recovery totals

OpenCode harness lifecycle
  ├─ start_inner() → register_opencode_server(pid)
  └─ stop()        → unregister_opencode_server(pid)
```

## Risks and bottlenecks

- **PID reuse** — if a PID is reused by an unrelated process that happens to contain "opencode" and "serve" in its command line, it would be killed. The double-check (ownership + process name) makes this extremely unlikely but not impossible.
- **Registry file races** — register/unregister do read-modify-write on a JSON file without locking. Safe when a single `lfd` process owns all writes (the intended deployment), but concurrent `lfd` instances could corrupt the file. Not a current concern since only one `lfd` runs per host.
- **macOS-only process checks** — `kill -0` and `ps -o command= -p PID` are POSIX but the exact behavior may differ on Linux. CI runs on ubuntu-latest; the unit tests use injected closures so they pass everywhere. The live code paths only run on macOS today.

## What's not included

- No reconnect/resume for orphaned sessions — they're failed, not recovered.
- No generalized orphan cleanup for Claude or Codex harnesses (they don't spawn child servers).
- No file locking on the registry (single-writer assumption holds).
