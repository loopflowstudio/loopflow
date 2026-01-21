---
status: proposed
area: infra
---

# Add lfd health checks and watchdog

`lfd` is designed to run unattended, but today there is no reliable signal for whether it is alive, stuck, or repeatedly crashing. Add a lightweight health model and a supervised run mode so background loops are observable and self-recovering.

## Scope

- What's included
  - Heartbeat and health state persisted to a well-known file (e.g., `~/.lf/lfd/health.json`)
  - `lfd status` reporting healthy/degraded/offline with last tick time, active loops, and last error
  - Supervised run mode that restarts the daemon with exponential backoff and records restart counts
  - Log rotation for `lfd` output to prevent unbounded disk usage
- What's explicitly not included
  - A GUI dashboard or Maestro integration
  - Cross-host orchestration or remote health checks

## Approach

- Add an `LfdHealth` record updated on each scheduler tick (timestamp, active loops, last error, last successful task)
- Persist health atomically (write temp + rename) to avoid partial reads during updates
- Extend `lfd status` to read and interpret health, falling back to offline if the file is missing or stale
- Implement `lfd run --supervised` that spawns a child process, monitors exit codes, and restarts with backoff
- Rotate logs by size and keep a small fixed window of recent logs for debugging
- Cover the health file update and stale detection logic with unit tests
