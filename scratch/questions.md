# Rebase notes — 2026-04-23

## Situation

Branch `jack-heart.flows.20260423_1414` rebased onto `origin/main`. The rebase
succeeded, but the cumulative diff vs `origin/main` is **zero**.

The branch's work was already squash-merged to main as commit `5803e553`
("flows: ship the build/govern/ops catalog in Concerto"). All four conflicted
files (`README.md`, `swift/LoopflowCore/Models/Catalog.swift`,
`swift/LoopflowCore/Services/LocalWaveService.swift`, `wave/flows/README.md`)
had stale branch versions vs newer shipped versions on main — in each case
`--ours` (main) was taken, since main had the polished post-merge content.

After rebase, the 5 remaining commits only touch `scratch/` files, with the
final `lf land: clear scratch/` commit removing them all.

## Assumption

Pushed the rebased branch with `--force-with-lease`. The next `lf op` action
(land/prune) should recognize the branch as merged and clean up the worktree.
No tests run because there is no code delta to test.
