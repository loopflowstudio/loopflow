# Worktree machinery

The stack primitives hold under real use. Audited 2026-07-08:

## KRs

- Stacking persists child base SHA at creation — OPEN: the start point is
  computed transiently (lfd/executor/helpers.rs:226) and never recorded; no
  base_sha field on Run or in the schema, so re-parenting re-infers via
  patch-id scans.
- Stacked re-parent onto main when the parent merges — SHIPPED with tests
  (#836; ops/rebase.rs:98, lfd/queue.rs:900+). Remaining: one live-run
  verification; overlapping reworked parents surface RebaseConflict by
  design.
- `lf op next` works from the wave home — root cause found: reset_to_main
  runs `git checkout main` (ops/next.rs:163) but the canonical checkout
  owns main; fix is re-baselining against origin/main without occupying
  the branch.
- Wave "home" cross-machine (mac-mini) — GREENFIELD; only stateless
  `lf ssh` exists. Answer the question before building.
