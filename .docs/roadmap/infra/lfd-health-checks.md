---
status: proposed
area: infra
---

# Daemon health checks and self-heal

Background agents are core to Loopflow, but the current daemon behavior has no explicit health contract. Add a lightweight health-check and recovery path so loops are observable, failures are surfaced, and stuck daemons can be recovered without manual cleanup. This improves reliability for headless runs and reduces developer toil.

## Scope

- Define a health check for `lfd` (last heartbeat, active loops, last error)
- Persist daemon state to a single on-disk status file for `lfd status` and future UI
- Add a watchdog path that can detect a stalled daemon and restart cleanly
- Emit a clear notification on failure and on automatic recovery
- Add tests around heartbeat updates and status parsing

- Not included: full metrics/telemetry backend, remote monitoring service
- Not included: changes to loop scheduling semantics or run mode behavior

## Approach

Create a small `DaemonStatus` record (json) stored under `~/.lf/state/lfd.status.json` with timestamped heartbeat, active loop IDs, and last error. Update it on daemon start, loop start/stop, and on a fixed heartbeat interval. Add `lfd status --health` to report OK/warn/stale based on configurable thresholds. Implement a watchdog in `lfd` startup that checks for an existing pid + stale heartbeat; if stale, archive the old status and start fresh. Use the existing notification mechanism to surface failure/recovery. Tests should mock time and file IO to validate stale detection and status parsing logic.
