# ab74ab: lf sync re-places a Task whose stack parent PR closed unmerged

## Outcome

A Task stacked on a PR that closed without merging syncs again:

- bare `lf sync` (and the Graph `cmd: sync` step, the same `run_sync_in`)
  follows the parent Task to its current PR, merges it, and records it as the
  parent;
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

## The model (concept review, 2026-11-19)

A person stacks a Task on a **Task**. The record names a **PR attempt**
(`task_checkouts.parent_pr_id` → one `task_prs` row). When the parent reopens,
the person's parent is unchanged; the record's parent is dead. The fix is to
make every read resolve the parent the way the person thinks of it:

> The stack parent's PR is the recorded row, unless it closed without merging;
> then it is the parent Task's active row. Closed only when that is closed too.

`select_stack_parent` already applies this rule when stacking; `task_stack`
does not. Both read one function. Following needs no write on the read: the
sync that merges the followed PR records it, the way it already records the
base it reached.

Chosen for review (the earlier plan had a separate `restack_closed_parent`
pass in `run_sync_in` that called `stack_existing_task` and re-read
`task_stack`; passed over because it adds a second re-placement path and leaves
landing and publication refusing a parent that is live).

## Design

`rust/loopflow-execution/src/ops/task.rs`:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StackParentPr { Open, Merged, Closed { number: Option<u64> } }

pub struct StackedSync {
    pub fork_base: String,
    pub child: Task,
    pub parent_identifier: String,
    pub parent_branch: String,
    /// The row the parent resolves to now; differs from the child's recorded
    /// `parent_pr_id` when the recorded PR closed and the parent reopened.
    pub parent_pr: TaskPrId,
    pub parent: StackParentPr,
}
```

- `current_parent_pr(store, recorded: TaskPr, parent_task) -> TaskPr`: the
  rule above. `task_stack` calls it after `reconcile_task_pr`;
  `select_stack_parent` uses it in place of its own abandoned check.
- `task_stack` never errors on a closed parent. A followed successor that
  merged yields `Merged`, so bare sync takes the existing merged-parent path
  onto the default branch. `fork_base` is unchanged: the recorded base is on
  the parent's branch, which the successor row shares.
- `live_parent_branch()` is `Some` only for `Open`.
- `StackedSync::closed_parent_error()` writes the sentence above. Every
  consumer that needs a parent and gets `Closed` returns it: `run_sync_in`
  when not detaching, `stack_for_landing`, `pr.rs::publication_base`.
- `record_stack_sync`: when not clearing and `parent_pr` differs from
  `child.parent_pr_id`, record `parent_pr` with the new base in the same write.
  `record_control_task_state`'s `clear_parent` reads `parent == Merged`.
- `run_sync_in` (`sync_execution.rs`) changes only by the `Closed` error.
  `lf sync main` is the existing detach; `lf sync <Task>` is the existing
  `open_task_named` + `stack_task`.

Followed rows count when unpublished (no GitHub PR yet), the same rule
`select_stack_parent` already uses.

## Acceptance check at gate

```
cargo test -p loopflow --test integration sync_tests::stacking_retry_replaces_an_abandoned_parent_and_syncs
```

Rewritten from the existing test of that name (`rust/loopflow/tests/sync_tests.rs:839`),
which today asserts the refusal. For each of bare `lf sync` and
`lf sync INF-123`: close the parent's PR row, insert a successor row with
GitHub number 43, run the sync, and assert the parent's file arrives,
the child's recorded parent is the successor row, and
`lf task status INF-124 --json` has `stack_parent.pr_number == 43`. A third
case with no successor asserts bare sync's error names both commands, then
`lf sync main` detaches (`task_stack` is `None`). The `lf checkout --stack-on`
retry stays covered by its own assertions.

## What goes

- The `closed && !merged` early `Err` in `task_stack`.
- `StackedSync.parent_merged` (replaced by `parent`).
- The abandoned-row check inside `select_stack_parent` (now `current_parent_pr`).
- The `unwrap_err().contains("closed without merging")` assertion in
  `sync_tests.rs`.

## Exclusions

- `lf task status` before a sync still shows the recorded #1971.
- A parent branch deleted on close: GitHub closes the child PR with it.

## Built (implement, 2026-11-19)

Slices 1 and 2 in one pass: `StackParentPr`, `current_parent_pr`, the
`Closed` sentence in `run_sync_in`, `stack_for_landing` and
`publication_base`. `sync_task_placement` now takes the parent to record
(`Option<&TaskPrId>`) instead of a clear flag, so the followed row and base
land in one write. `select_stack_parent` is unchanged: its fall-through to
the parent's active row already is the rule, so a call to
`current_parent_pr` there would add nothing. Docs (slice 3) already match the
built sentence.

Checks: `cargo test -p loopflow --test integration
sync_tests::stacking_retry_replaces_an_abandoned_parent_and_syncs` passed
2026-11-19 (realign; all five cases incl. Detach). `task_pr_authority_tests`,
`store::` placement tests and clippy: gate/CI.

Remaining: none in implement; gate next.

## Slices

1. **The fixture first.** Rewrite the test above and watch it fail; add
   `current_parent_pr`, `StackParentPr`, the record in `record_stack_sync`, the
   `Closed` error in `run_sync_in`. Shows the whole Done-when.
2. Landing and publication return the shared sentence on `Closed` and follow a
   live successor. Focused: `task_pr_authority_tests`, the stacking tests in
   `sync_tests.rs`.
3. Docs (written ahead, write-docs 2026-11-19): `docs/ops.md` Checkouts gains
   the follow paragraph, example and the closed-parent error; `docs/work.md`
   stacking paragraph gains one clause. Match them to the built messages.

## Demo

On LOO-995's checkout after install: `lf sync` prints `Synced onto
origin/<LOO-938 branch>.`; `lf task status LOO-995 --json` shows
`stack_parent.pr_number: 1983`.
