# LOO-837 recovery — 2026-11-13

Jack Heart: "Next: merge main into the branch, read the 2 failing checks from CI and fix them directly in this checkout, push, and stop at demo."
No demo occurred: this run is headless. No reactions or approval were collected.
The stopped ship loop and personal configuration stay untouched.

## Outcome and approach

Stack on any placed Task branch, pushing to origin without force when absent or
behind. Record an unpublished Task PR when needed; open no parent GitHub PR.
Share parent selection between new and existing placements. Diverged origin
remains the child's base and the stack message counts omitted local commits.
Placement without a Task PR is intentional; publication owns its record until
stacking needs to refer to it. Keep retry identity and squash-parent sync.

## Delete — do not maintain

The separate existing/new stack selectors and published-parent preconditions
were replaced by `select_stack_parent` / `record_stack_parent`. Keep that single
path and coverage for unpublished parents, push rejection, divergence and sync.
The abandoned-specific refusal in `select_stack_parent` is also removed;
the existing no-PR placement and recording path handles a closed parent PR.

## Current findings

PR #1888 head `205b16bd25` conflicted with main. CI run 43033634214,
job 130569733554: 519/520 tests passed; the admission migration fixture timed
out at 120.012 s. Job 130577057003 (`tests-result`) only aggregates that failure.
Main `39b3779427` already includes `3f3153d95c`'s targeted fixture repair:
construct the historical schema once and copy it per case, retaining all 17
independent populated upgrades and assertions. The merge retains that repair.
Its sole conflict, `docs/work.md`, keeps main's `--open` example and this
branch's placed-parent wording. Prior QA publication feedback is in `e6c5971f1e`.

Auto-merge was still enabled on #1888 from the earlier delivery attempt; it
is now disabled so a recovery push cannot merge before demo.

Checks:
- `cargo test -p loopflow --lib linear::acceptance::tests::task_admission_reads_released_evidence_without_reconstructing_provider_content`: passed all 17 cases in 67.83 s after the merge (old CI timeout: 120.012 s). macOS linker warned about the debug test binary's unwind section size.

- Built library binary, filters `ops::task::tests::task_preparation_stacks` and `ops::task::tests::stacking_retry_keeps_a_merged_parent`: 3 passed in 10.70 s (unpushed/unpublished parent, divergence, settled-parent retry).
- `cargo fmt --all -- --check`: passed.

- `cargo test -p loopflow --test integration -- documented_commands:: sync_tests::stacking_on_an_unpushed_parent_pushes_its_branch_and_sync_follows_it`: 5 passed in 170.39 s; CLI branch publication, parent link and sync verified, along with maintained command examples.

- `cargo clippy --all-targets -- -D warnings`: passed.
- `git diff --check` for the Task's changes: passed. The staged merge includes existing main-only whitespace in patch assets and a released migration; those files are unchanged by this Task.

Published `f7b316e62ac6624a3e63ab5d36ee5e72429d49a6` to PR #1888.
GitHub reported `MERGEABLE`, auto-merge disabled, and CI run 43540649413 started.
PR copy records the recovery checks and remaining provider/GitHub demo limits.

Later push (seen at realign): head `fbedb8ce21` = checkpoint `3050ad59c3`, which
committed this scratch note, plus a clean merge of main `d981a4330f`. PR stays
`MERGEABLE`, auto-merge off. Because scratch/ is no longer clear, CI run
43540875708's `scratch-clear` marks the PR a non-candidate and the hosted suites,
including the admission fixture, are deferred: no hosted run has yet proved the
timeout repair. Hosted proof needs a head with scratch/ clear (`lf pr submit`
clears added scratch).

Remaining: demo judgment; a scratch-clear head so hosted CI runs the full matrix. No demo or
approval occurred in this headless recovery. The stopped ship loop stays stopped.

Compress (after the recovery push): no edits. The diff already runs one stack
path (`select_stack_parent` / `record_stack_parent`, shared by placement and
`checkout --explain`); main's new `d981a4330f` merges clean
(`git merge-tree --write-tree HEAD origin/main`), so nothing was rebuilt or rerun.

## Concept review (2026-11-13, after recovery) — implemented direction

The rule is "the parent is a Task with a placed branch". One refusal in
`select_stack_parent` contradicts it: an active PR in phase `Abandoned` errors
with "PR was abandoned; it has no live branch to stack on" without looking at
the branch. `lf pr abandon` does delete the branch and worktree, so there the
claim holds. A PR closed on GitHub leaves both: the parent Task is still placed,
publication treats that record as spent and makes the next one
(`request_task_pr_publication` filters settled, `next_task_pr`; `insert_task_pr`
retires the settled row), and stacking refuses with a statement that is false.

Change: an `Abandoned` active PR counts as no PR record. The existing
no-placed-branch check then decides: placed branch → stack, pushing and
recording the next unpublished PR exactly as for a parent that never published;
no worktree or branch → the existing "has no placed branch; run `lf checkout`"
error. The abandoned-specific error is deleted. `StackParent::branch()` already
falls back to the Task's branch when `pr` is `None`. One test: parent with a
closed PR and a live branch; the child stacks and the parent gets a new
unsettled row with the old one historical. Docs list "already merged" among the
stops and never mention abandoned, so they stay.

Kept, with its counterexample: a `Merged` parent still refuses ("already merged;
place this Task without --stack-on"). A parent that keeps working after its
merge toward a second PR also has a placed branch, but its branch still carries
the pre-squash commits and sync's merged-parent path sends the child to main;
telling those apart needs a "has new work since the merge" reading that nothing
has asked for. Not changed here.

No open product choice.

Implement check: `cargo test -p loopflow --lib ops::task::tests::task_preparation_stacks_on_a_placed_parent_with_an_abandoned_pr` passed (1 test); `cargo fmt --all -- --check` and `git diff --check` passed. The fixture uses its persisted timestamp because SQLite truncates subsecond precision. The existing macOS unwind-section linker warning remains; gate owns clippy and hosted suites for this revision.

Compress (after the abandoned-PR change): the parent PR's phase is read in one
`match` in `select_stack_parent` (Merged refuses, Abandoned counts as none), and
the new test drops two assertions its sequence and old-row checks already imply.
`cargo test -p loopflow --lib ops::task::tests::task_preparation_stacks`: 3 passed in 1.80 s; `cargo fmt --all` clean. Clippy not rerun on this revision; gate owns it.
