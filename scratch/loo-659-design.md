# LOO-659: sync never leaves step edits only in a stash

Status: implemented 2026-10-27.

## Problem

A Graph's `cmd: sync` runs `lf sync` over an implement step's uncommitted edits.
`with_preserved_edits` stashed them, merged main (often a fast-forward, since the
branch had no commits), then `git stash apply --index` conflicted after the
resolver had already finished. The route failed with the work only in the
shared stash, and `run_task_edge_for` correctly refused to retry over it.

Evidence:
- LOO-625: Graphs 80f173bf (stash 8e57dd39) and 1d7b0418 (stash 60764eff);
  README.md, docs/lf-reference.md, ops/task.rs, two upstream-deleted docs.
- LOO-651 (Jack Heart's steer): Graphs 34ea51c6 and ef5f5d46, stashes b00c6ae7
  and 887b0347; both docs/lf-reference.md (Task removes generated rows, main
  edits one). Partial apply staged every tracked edit though the original index
  held one deletion; large untracked modules/workflows/docs. Recovery restored
  index layout and untracked contents by hand, then committed before sync
  (8d6fd07572); the next sync passed.

## Decision

Agent-assisted `lf sync` (not `--manual`, not on the default branch) commits
uncommitted edits as "Checkpoint edits before syncing onto <base>" before a
MergeTarget sync, then re-plans so the recorded plan is the one that runs.
Conflicts become ordinary merge conflicts handled by the existing
`sync-conflicts` resolver inside the owned sequencer. The index layout is not
preserved separately: everything the step produced is committed together.

Unchanged: ResetToBase plans (scratch-only/stale branches) keep their stash
boundary; `refresh_main` and `restart_landed_branch` keep `with_preserved_edits`;
`retained_edits` still stops a Task retry when one of those leaves a stash.

## Checks

- `sync_tests::uncommitted_edits_that_conflict_with_main_reach_the_resolver_not_a_stash`, `saved_graph_sync_follows_task_parent…`, two dirty-branch sync tests: pass; clippy clean. Full sync suite left to gate.

The checkpoint stages everything except `.lf/tmp/` (telemetry is not work even
where a repository doesn't ignore it; the parent-sync test caught this).
