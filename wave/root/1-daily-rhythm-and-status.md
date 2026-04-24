---
asana_id: '1214270115593678'
---
# Daily rhythm and status

**Finish line:** Manual `review-open-work` and automated `govern-*` / `garden-*` flows share surfaces and vocabulary. The conductor has one morning ritual — open Concerto, see the full overnight picture (shipped PRs, pending chord proposals, calibration checkpoints, anything that needs manual attention) in one place. Not two parallel systems; one rhythm.

## Context

`review-open-work` is manual: the conductor walks branches, PRs, worktrees, and waves in two passes. Automated `govern-coordination`, `govern-identity`, `govern-intelligence`, `govern-control`, and `garden-act` run on cron, each producing structured observations and mutation proposals via `wave/mutate`.

Today the two families produce different artifacts with different vocabulary at different altitudes. The conductor switches mental modes between them. That's friction, and it hides when the automated rhythm is actually healthy.

## What to shape

- **Shared output format** — manual and automated status produce the same kinds of signals: wave health, attention items, proposed mutations, calibration notes
- **Runboard as canonical surface** — Desktop's runboard (see `desktop/conductor-surfaces`) shows overnight output from scheduled flows alongside active attention
- **Sensible schedule** — `govern-coordination` daily, `garden-act` on its own cron, `review-open-work` on demand. Picked to surface real drift without creating churn.
- **On-demand trigger** — manual `review-open-work` kicks off a fresh govern/garden pass so the conductor's status check is reading current signals

## Daily experience

Morning coffee. Open Concerto. Runboard home: 3 PRs shipped overnight (loop ran clean), 1 mutation PR from garden cycle ("split `desktop/conductor-surfaces` — it's grown to 4 items"), 1 wave blocked on CI, 0 calibration checkpoints. 10 minutes of review + approval. Laptop closes. System keeps running.

## Done when

- Manual and automated modes produce compatible output
- One Concerto surface (runboard) shows both
- Schedule feels right — no over-churn, no stale signals
- Manual review can trigger a fresh scan/assess pass on demand
