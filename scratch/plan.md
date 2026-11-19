# ab74ab: lf sync re-places a Task whose stack parent PR closed unmerged

## Outcome

A Task stacked on a PR that closed without merging syncs again:

- bare `lf sync` (and the Graph `cmd: sync` step, which is the same
  `run_sync_in`) follows the parent Task to its current PR, records it as the
  new parent, retargets the child's open PR, then merges it;
- `lf sync <parent Task>` stacks on the parent's current PR;
- `lf sync main` detaches it.

With no PR left to follow, bare sync fails with one sentence naming both
commands, so the Graph step's three retries end on something a person can act on:

> stack parent LOO-938's PR #1971 closed without merging; `lf sync LOO-938` restacks this Task on its branch, `lf sync main` detaches it

## What the real case shows (probed 2026-11-19, `~/.lf/loopflow.db`, read-only)

- LOO-938 has two `task_prs` rows: seq 1 #1971 `abandoned_at` 2026-11-19 01:35,
  seq 2 #1983 open, created 01:38. A reopened PR is a new row, not a rewrite.
- LOO-995's `task_checkouts.parent_pr_id` still names the #1971 row.
- LOO-996, stacked after the close, names the #1983 row; it syncs fine.
- `~/.lf/logs/graph-*.log` holds the error from the `implement`→`sync` retries.

`select_stack_parent` (`ops/task.rs`) already skips a recorded parent row in
`PrPhase::Abandoned` and falls through to the parent's `active_task_pr`, so at
this base `lf sync LOO-938` should get through `stack_task`. It still dies
after, or did on the installed build. The fixture in slice 1 settles which;
either way the refusal in `task_stack` goes.

## Source of truth

`task_checkouts.parent_pr_id` → a `task_prs` row. `task_stack`
(`rust/loopflow-execution/src/ops/task.rs`) is the one resolver every consumer
reads: sync, owned-sync recording (`sync.rs::record_control_task_state`),
landing (`stack_for_landing`), publication (`pr.rs::publication_base`). It
reconciles the parent's PR and must stay read-only ("without changing
placement on a read").

## Design

`StackedSync.parent_merged: bool` becomes `parent: StackParentPr`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StackParentPr { Open, Merged, Closed { number: Option<u64> } }
```

- `task_stack` returns `Closed` instead of an error. Fork base resolution is
  unchanged (recorded base, else merge-base with `origin/<branch>`).
- `live_parent_branch()` is `Some` only for `Open`.
- One function in `ops/task.rs` writes the sentence above
  (`StackedSync::closed_parent_message()` or similar); sync, landing and
  publication all print it. Landing and publication keep refusing on `Closed`:
  their base is unknown until a sync re-places the Task.
- New `restack_closed_parent(worktree, &StackedSync) -> OpsResult<String>` in
  `ops/task.rs`: reads the parent Task's `active_task_pr`; when it is neither
  merged nor abandoned, runs `stack_existing_task(store, main, child,
  parent_identifier)` (records the parent, recomputes base from shared
  history because the row id changed, retargets the child's open PR) and
  returns its "Stacked … on …" line; otherwise returns the error.
- `run_sync_in` (`sync_execution.rs`): after `task_stack`, when the parent is
  `Closed` and `onto` is `None`, print `restack_closed_parent`'s line and read
  `task_stack` again. `lf sync main` takes the existing detach path, which
  needs only `fork_base`. `lf sync <Task>` already goes through
  `open_task_named` + `stack_task` before `task_stack`.
- `record_control_task_state`: `clear_parent` reads `parent == Merged`.

Chosen for review: following an unpublished (not yet on GitHub) successor
row counts as "current PR", the same rule `select_stack_parent` uses. Passed
over: following only a GitHub-open PR, which would leave a Task stuck while
its parent republishes.

Passed over: making `task_stack` itself follow the successor. It is read by
status and publication, and re-placing there turns a read into a write.

## Acceptance check at gate

```
cargo test -p loopflow --test integration sync_tests::stacking_retry_replaces_an_abandoned_parent_and_syncs
```

Rewritten from the existing test of that name (`rust/loopflow/tests/sync_tests.rs:839`),
which today asserts the refusal. For each of bare `lf sync` and
`lf sync INF-123`: close the parent's PR row, insert a successor row with
GitHub number 43, run the sync, and assert the parent's file arrives,
`task_stack` names the successor row, and `lf task status INF-124 --json`
has `stack_parent.pr_number == 43`. A third case with no successor asserts
bare sync's error names both commands, then `lf sync main` detaches
(`task_stack` is `None`). The `lf checkout --stack-on` retry stays covered by
its own assertions.

## What goes

- The `closed && !merged` early `Err` in `task_stack`.
- `StackedSync.parent_merged` (replaced by `parent`).
- The `unwrap_err().contains("closed without merging")` assertion in
  `sync_tests.rs`.

## Exclusions

- `lf task status` saying the parent PR closed (it shows the recorded #1971
  until a sync re-places it).
- A parent whose successor PR already merged: bare sync gives the two-command
  sentence; `lf sync main` handles it.
- A parent branch deleted on close: GitHub closes the child PR with it; not
  this Task.

## Slices

1. **The fixture first.** Rewrite the test above against the current code and
   watch which sync fails where; then drop the refusal, add `StackParentPr`
   and `restack_closed_parent`, wire `run_sync_in`. Shows the whole Done-when.
2. Landing and publication print the shared sentence; update the
   `record_control_task_state` read. Focused: `task_pr_authority_tests`,
   the stacking tests in `sync_tests.rs`.
3. `docs/work.md` stacking paragraph: one line that a closed parent PR is
   followed to its next PR by `lf sync`.

## Demo

On LOO-995's checkout after install: `lf sync` prints
`Stacked LOO-995 on LOO-938; …` and merges; `lf task status LOO-995 --json`
shows `stack_parent.pr_number: 1983`.
