# Assumptions

- `PmRefresh::Auto` treats snapshots up to 15 minutes old as fresh, permits a
  cached fallback up to 24 hours old when refresh fails, and rejects older
  snapshots. The design requires bounded fresh/soft-stale/hard-stale behavior
  but does not specify durations; these fixed values avoid a new config knob.
- The existing Linear marker in `task start` is the idempotency receipt for the
  current provider API. If task creation commits but snapshot refresh fails, a
  retry finds the same marked issue before attempting another create.
