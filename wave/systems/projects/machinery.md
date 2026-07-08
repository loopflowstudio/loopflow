# Machinery

The primitives under every run — worktrees, stacking, verification — hold
under real load. Audited 2026-07-08; each item carries its verdict.

## KRs

- Stacking persists child base SHA at creation — OPEN: computed transiently
  (lfd/executor/helpers.rs:226), never recorded; re-parenting re-infers via
  patch-id because of it.
- Stacked re-parent onto main when the parent merges — SHIPPED with tests
  (#836); remaining: one live-run verification.
- `lf op next` works from the wave home — root cause known: reset_to_main
  checks out main, which the canonical checkout owns; fix is re-baselining
  against origin/main without occupying the branch.
- Wave-home cross-machine (mac-mini) — GREENFIELD; answer the question
  before building.
- Local verification (the full pre-land matrix) gets a measured baseline
  and budgets; GitHub CI holds under 2.5m median as a regression bar
  (already ~1m55s — if the local baseline also comes back fast, delete
  this KR).
