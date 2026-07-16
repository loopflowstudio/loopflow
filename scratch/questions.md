# Open questions

## Pre-existing `land_tests` failures (not caused by this branch)

`latest_land_disposition_wins_before_merge` and `lf_ops_land_leaves_worktree_in_place`
in `rust/loopflow/tests/land_tests.rs` fail on the baseline (verified by stashing
this branch's changes and running the tests on clean HEAD). Both fail with
"ambient Task Session ts_... is not registered" — a test-harness registration
issue unrelated to session body resolution. Out of scope for this work.

## Base commit `318c87ae9` bundles changes outside session bodies

The pushed base commit `318c87ae9` (authored before this working session) changes
files that are not session-body launch resolution: `lf/commands/receipt.rs` and
`receipt.rs` removal, `lf/mod.rs`, `ops/flow.rs`, `ops/pm.rs`, `store/sqlite.rs`,
`swift/**` HandoffSurface removal, `scripts/screenshots.*`, and `wave/*/GOAL.md`.
The "final PR contains only session-body changes" directive was satisfied for the
W2-206 parent commits (excluded via `rebase --onto`), but this pre-existing base
still carries the above. Removing them risks destroying legitimate sibling work,
so it is flagged here for a human to confirm scope rather than silently reverted.
My design corrections (current-Home resolver, no `ChildExecutionContext` domain
state, boot-time provenance) are a focused follow-up on top of that base.
