# LOO-968 open choices (kickoff, 2026-11-18, headless)

- Stop doesn't kill the run; it stops at the next interactive step or landing. Simpler than today's kill. Reverse if a stop must be immediate.
- `ship_settle_merge` goes: a hand-merged PR is met by the Graph's land step. Check the land step really returns success on an already-merged PR before deleting.
- Auto mode survives taking up another graph (columns stay on the row).
- Migration NULLs `tasks.shipped_at` so released lf stops relaunching `lf ship` during upgrade skew.
- Branch stacks on LOO-959 (#1972); rebase onto main if it merges first.
