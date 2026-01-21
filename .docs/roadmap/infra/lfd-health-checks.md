---
status: proposed
area: infra
---

# LFD Loop Health Checks and Alerts

Add first-class health signals for background loops so failures and stalls surface quickly. This improves reliability for unattended runs and reduces developer time spent polling logs or manually checking loop status.

## Scope

- Emit per-loop health status (healthy, stalled, failed) with last-progress timestamps
- Surface health in `lfd status` and `lfd prs` output
- Optional local notifications on health transitions (reuse existing notify hooks)
- Configurable thresholds for stall detection and retry limits

- No hosted telemetry or SaaS dashboard
- No remote alert routing (Slack, email, etc.)

## Approach

Persist health state alongside loop metadata (e.g., `.lf/loops/<id>/health.json`) with timestamps for last step start/finish and last successful iteration. Update the loop runner to mark a loop as stalled when no progress occurs within a configurable window, and failed after repeated iteration errors. Extend `lfd status` to show health with a short code and include the last-progress time for quick triage. Add a small notification hook that fires on transitions to stalled/failed, reusing existing local notification mechanisms for consistency.
