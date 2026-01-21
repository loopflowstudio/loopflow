---
status: proposed
area: infra
---

# LFD Health Checks and Stuck-Loop Recovery

The background daemon is core to Loopflow's promise of tight, reliable loops. Today, when a loop stalls or a worktree becomes inconsistent, the failure mode is silent and requires manual cleanup. Add explicit health checks and automatic recovery so long-running loops stay trustworthy and operators get fast signal when intervention is needed.

## Scope

- Add daemon heartbeat tracking and a health status snapshot on disk
- Detect stalled loops and surface clear status in `lfd status`
- Provide a safe auto-recovery path (retry with backoff, then halt and report)
- Document rollback and manual recovery steps for operators

- Not included: Maestro UI changes, notifications routing, or cloud-hosted daemon

## Approach

Introduce a lightweight health registry under `~/.lf/state/` that records per-loop heartbeat timestamps, last successful step, and retry state. Extend `lfd status` to read this registry and flag stalled loops with actionable messages. Implement a recovery policy: on missed heartbeats, retry the last step up to N times with exponential backoff, then mark the loop halted and emit a log entry with a clear remediation hint. Keep the policy configurable in `.lf/config.yaml` with conservative defaults and ensure all new failure states are surfaced in logs for observability.
