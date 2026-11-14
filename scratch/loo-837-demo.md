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

## Abandoned parent PR (concept review, 2026-11-13) — done in `dbde53699f`

The rule is "the parent is a Task with a placed branch". A PR closed on GitHub
leaves the parent's branch and worktree, so `select_stack_parent` now treats an
`Abandoned` active PR as no record: placed branch → stack and record the next
unpublished PR (old row stays historical); no branch → the existing "has no
placed branch; run `lf checkout`" error. `lf pr abandon` deletes the branch, so
it lands in that error. Docs never mentioned abandoned; unchanged.

Kept, with its counterexample: a `Merged` parent still refuses. A parent working
toward a second PR after its merge still carries pre-squash commits, and sync's
merged-parent path sends the child to main; telling them apart needs a "new work
since the merge" reading nothing has asked for.

Checks on `dbde53699f` (pushed; merges clean with origin/main):
- `cargo test -p loopflow --lib ops::task::tests::task_preparation_stacks`: 3 passed in 1.80 s, including `..._on_a_placed_parent_with_an_abandoned_pr` (new unsettled parent row, sequence +1, old row unchanged). The fixture uses its persisted timestamp because SQLite truncates subsecond precision.
- `cargo fmt --all -- --check`, `git diff --check`: passed.
- Clippy not rerun on this revision; gate owns it with the hosted suites.

No open product choice. Remaining: gate (clippy, scratch-clear head so hosted CI
runs the full matrix including the admission fixture), then demo.
