# LOO-578: Graph sync reset a Task branch and stranded its work

## Delivered (PR #1672, merged 0b17cf980f)

- `GeneratedOnly` deleted: a branch with its own commits always merges its target (1).
- `lf sync` plans once, with the caller's edits present, and runs that plan (1, 2).
- `lf task run` stops instead of restarting a Graph whose checkout holds a
  checkout-update stash, and prints the restore command (3).
- Tests: `graph_sync_keeps_an_unpublished_scratch_commit_and_its_edits`,
  `sync_runs_the_strategy_it_records_for_a_dirty_branch` (`tests/sync_tests.rs`),
  `a_graph_that_fails_with_retained_edits_is_not_started_again`
  (`tests/linear/task_graph_launch_tests.rs`).
- Incident analysis (5 whys): git `e5778d0c50^:scratch/loo-578-sync-reset.md`.

## On this branch, unpublished

- `012994a701` compress: `sync_with_plan` borrows the `SyncPlan` instead of
  cloning it. No behavior change. Remaining: publish and land it.

## Elsewhere

- (4) Interrupt that doesn't stop a Task Graph: LOO-579 (started).
- (2b) A failed restore doesn't move HEAD back. Not pursued: the resets that
  remain only move HEAD forward, so the stash applies as an ordinary edit.

## Check

Gate (2026-10-24): `cargo fmt --all -- --check` pass; `cargo clippy -p loopflow --all-targets -- -D warnings` pass;
`cargo test -p loopflow --test integration sync_` 50 passed. Remaining suites left to CI.
