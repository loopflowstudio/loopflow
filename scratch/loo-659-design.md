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
The checkpoint stages everything except `.lf/tmp/` (telemetry is not work even
where a repository doesn't ignore it; the parent-sync test caught this).

Unchanged: ResetToBase plans (scratch-only/stale branches) keep their stash
boundary; `refresh_main` and `restart_landed_branch` keep `with_preserved_edits`;
`retained_edits` still stops a Task retry when one of those leaves a stash.

## Concept review (2026-10-27, Graph, no person)

The change removes a concept rather than adding one: step edits no longer have
a "held in a stash across a merge" state on the MergeTarget path; they are
commits, and conflicts have one owner (the `sync-conflicts` resolver). Checked:
- Normal path: dirty Task branch → checkpoint commit → merge → push. Matches
  the Task's suggested `lf commit` direction; no new flag or mode.
- Failure: resolver can't settle the merge → the ordinary sequencer failure, with
  the work in a branch commit, so a Task retry starts from a checkout that has it.
- Recovery: `retained_edits` still guards ResetToBase, `refresh_main` and
  `restart_landed_branch`, the paths that still stash; `with_preserved_edits`
  stays live for `--manual` and default-branch syncs.
- The checkpoint commit is pushed with the branch; scratch it contains is still
  removed by `lf scratch clear` at submit/land.
No design change and no open product choice. The `scratch/questions.md`
assumption (interactive agent-assisted sync also checkpoints; `--manual` opts
out) is reversible and stands.

## Remaining

- Proposed infrastructure Wave memory (not written; standalone realign): a sync
  that restores stashed edits after merging fails once the resolver is gone;
  commit step edits before merging so conflicts reach the resolver.

## Checks

- Gate 2026-10-27: `scripts/test.py --reuse-passing --rust-filter 'test(sync_tests::) | test(reference_matches_the_compiled_cli) | test(documented_commands)'` rust tests pass; clippy and website were stopped by host syspolicyd pressure, then passed run directly (`cargo clippy --all-targets -- -D warnings`, `website/dev.py test` 72 passed). `--manual` help now says it keeps uncommitted edits local.

- `sync_tests::uncommitted_edits_that_conflict_with_main_reach_the_resolver_not_a_stash`, `saved_graph_sync_follows_task_parent…`, two dirty-branch sync tests: pass; clippy clean. Full sync suite left to gate.
