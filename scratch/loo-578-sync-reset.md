# LOO-578: Graph sync reset a Task branch and stranded its work

## Cause (1, 2)

`lf sync` without `--manual` (`lf/commands/ops/mod.rs`) stashes the caller's
edits through `with_preserved_edits`, *then* calls `sync_with_recovery`, whose
`start_owned_sync` re-plans on the now-clean tree. The outer plan saw the 21
edits (CleanAuthored → merge); the inner plan saw only the kickoff commit, all
added scratch files, and classified it `generated_only` → `ResetToBase`.
`reset_to_base` dropped the commit and rewrote its scratch notes as untracked
files. The stash was written against the kickoff commit, so applying it onto
`main` hit those untracked notes (and would otherwise be a modify/delete
conflict on the dropped scratch files). A reset that discards commits can never
restore edits made on top of them.

## Repair

- Deleted the `GeneratedOnly` class (a rebase-era heuristic from 6129aeb5ec).
  A branch with its own commits always merges its target; reset remains only
  for branches with no unique commits (`stale_empty`, `scratch_only`) and
  persistent branches whose commits have landed.
- `lf sync` plans once. It fetches the target, classifies against the tree
  holding the caller's edits, and passes that `SyncPlan` to `sync_with_plan`
  inside `with_preserved_edits`. The strategy that runs is the one the metric
  records and `is_avoidable_sync_class` judges. `sync_with_recovery` (no plan)
  still fetches and plans inside the owned sync for library callers.
- Recovery stashes name their checkout (`lf checkout update: caller edits in
  <path>`, matched canonically); `lf task run` stops instead of restarting a
  failed Graph when its checkout has one, and prints the restore command (3).

## Verification

- `graph_sync_keeps_an_unpublished_scratch_commit_and_its_edits` and
  `sync_runs_the_strategy_it_records_for_a_dirty_branch` (`tests/sync_tests.rs`);
  `a_graph_that_fails_with_retained_edits_is_not_started_again`
  (`tests/linear/task_graph_launch_tests.rs`). Focused runs and
  `cargo clippy --all-targets -- -D warnings` pass; full suites left to gate/CI.

Interrupt (4) is filed as LOO-579, "lf task interrupt stops a running Task
Graph". Launch it after this PR lands; it edits the same `run_for_task` loop.

## Open

- (2b) A failed restore still leaves HEAD wherever the update moved it. With
  commit-dropping resets gone, the remaining reset paths move HEAD forward
  only; returning HEAD on failure was not attempted.
- (4) `lf task interrupt` only appends a durable interrupt and disables
  automation; the Task-run retry loop and Graph driver did not observe it.
  Not reproduced here; needs its own investigation of the Graph's interrupt
  observation.

## Why it was possible (5 whys)

Affected: LOO-575's autonomous Graph. Its implementation sat only in stash
`08ebe9d2ca` until it was recovered by hand (`e1d2815de2`). Recovery is done.
On this branch the reset, the retry stop and the double plan are repaired and
tested; the interrupt is still open (LOO-579).

**A. The branch was reset (1).**
1. *Why reset?* The inner `plan_sync` in `start_owned_sync` classified the
   branch `generated_only` → `ResetToBase`.
2. *Why that class?* `e88c34a674` (2026-09-30, "Merge Task branch updates
   without rewriting authored history") widened the rebase-era test
   `generated_only(...)` to `scratch_only || generated_only(...)`. A commit
   that only adds scratch files then counted as disposable. Kickoff commits
   are exactly that, and they are the Task's design.
3. *Why did the inner plan see only scratch?* `lf sync` plans once against the
   real tree (`ops/mod.rs`, the `plan` used for the metric and
   `is_avoidable_sync_class`), stashes the edits through
   `with_preserved_edits` (`c56340a142`, 2026-09-23), then plans a second time
   against the cleaned tree. The class was chosen with the work hidden.
4. *Why did nothing catch it?* No test combined a committed branch with
   uncommitted edits and the non-`--manual` path. The sync tests check the
   classes on trees that are already clean. In production the
   `record_ops_metric` row records the **outer** plan
   (`clean_authored`/`merge_target`), not the reset that ran. The
   `telemetry-daily` check could not have seen any `generated_only` reset.

**B. The restore failed (2).** `reset_to_base` turns the dropped commit's
scratch files back into untracked files. The stash was written against the
kickoff commit, so applying it onto `main` collides with those files. Even
without the rewrite, it would be a modify/delete conflict. Any reset that
drops commits breaks the premise of `with_preserved_edits`, which is to
restore onto the commit the edits were written against.

**C. The Graph started over (3).** `run_for_task` restarted any Graph process
that exited 1 and never looked at the checkout's state. A sync failure that
leaves a retained stash is not transient: retrying on top of it strands the
work more deeply. The retry design assumed failed steps leave no side effects.

**D. The interrupt was ignored (4). This is inferred from code; the LOO-575
log was not re-read.**
- `task_interrupt` sets `automation_enabled=false` and appends a durable
  Interrupt event.
- `automation_enabled` is read only by scheduled automation
  (`task_automation`, `pr_landing`). The live `run_for_task` loop and the
  Graph runner never read it.
- The Interrupt event is read only by `observe_interrupt`, from a live agent
  turn (`task_input`). Each step captures its cursor in `read_seed` at
  launch. An interrupt that arrives between steps, during an op step such as
  `sync`, or during the retry gap is therefore absorbed into the next step's
  cursor and never acted on. "Interrupted" means "end the current turn", and
  a Graph is mostly not one turn.
- Only an interrupted turn ends a Graph (`StepExit::Stopped`). Nothing stops
  the Task run itself.

### Unanswered / contradicting evidence
- Whether LOO-575's interrupt landed during an agent turn. If it did and the
  turn still ended in a way that let the runner advance, D has a second cause:
  the headless exit code after an interrupted turn. Check the LOO-575 Graph
  log for the step that was live when the interrupt was posted.
- The `generated_only` subject heuristic (`checkpoint:`/`wip:`/"generated")
  was deleted along with the class. No caller still commits with those
  subjects (not verified across skills).

### Prevention (proposed unless marked done)
- **Done:** the `GeneratedOnly` class is deleted. A branch with its own
  commits always merges. Proof: `graph_sync_keeps_an_unpublished_scratch_commit_and_its_edits`.
- **Done:** `lf task run` stops when its checkout holds a checkout-update
  stash. Proof: `a_graph_that_fails_with_retained_edits_is_not_started_again`.
- **Done:** sync plans once, before the stash. Proof:
  `sync_runs_the_strategy_it_records_for_a_dirty_branch`. With `GeneratedOnly`
  gone, a committed branch classifies the same either way; the remaining
  divergence was a branch with no commits and dirty edits (recorded merge,
  executed reset), which the test covers.
- **Proposed:** make interrupt a Task-run control rather than a turn control.
  `run_for_task` and the Graph runner check `latest_interrupt_id` against a
  cursor captured at Task-run start, before each step and before each retry,
  and stop with exit 130. Proof: an interrupt appended between two op steps
  stops the Graph before the second one.
- **Not proposed:** returning HEAD on a failed restore (2b). Once
  commit-dropping resets are gone, the remaining resets move HEAD forward
  only, and the stash applies there as an ordinary edit.
