---
priority: high
---

# lf loop — standing maintenance crons

**Finish line:** A Wave carries standing crons that keep MEMORY fresh
out-of-band — daily full re-orient, periodic change-scan, regular rebase — so the
progress pass stays light and never re-orients from cold.

## Context

A Wave is a standing *system* of loops, not one loop (design:
`scratch/jack-heart.lf-loop.md`; makes `4-vsm-standing-loops.md` concrete).
Orientation is expensive to derive cold; pulling it onto crons that refresh
MEMORY means the progress loop reads fresh orientation and goes straight to
pick-blocker → dispatch. `cron.rs` is already a first-class trigger source and
waves carry a `crons` field.

## What to shape

- **orient-daily** — a full re-orient pass → refresh vision/state in MEMORY.
- **scan-changes** — periodic external-delta scan (main advanced? deps changed?
  sibling wave landed something?) → note deltas in MEMORY.
- **rebase** — keep the branch current on a schedule.
- Each cron = a thin `lf goal -b` pass with its role in the seed, writing MEMORY.
- Which crons a Wave gets by default vs. author-added ones.

## Done when

- A wave runs orient-daily + scan-changes + rebase on schedule, each refreshing
  MEMORY, and the progress pass demonstrably skips cold re-orient by reading what
  the crons wrote.
