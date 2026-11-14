# LOO-837 recovery — 2026-11-13

Jack Heart: "Next: merge main into the branch, read the 2 failing checks from CI and fix them directly in this checkout, push, and stop at demo."
No demo occurred: this run is headless. No reactions or approval were collected.
The stopped ship loop and personal configuration stay untouched.

## Outcome and approach

Stack on any placed Task branch, pushing to origin without force when absent or
behind, unless the parent's PR is published (see "Next implement direction"). Record an unpublished Task PR when needed; open no parent GitHub PR.
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

## CI recovery

CI run 43033634214 failed only the admission migration fixture (120.012 s
timeout). Main's `3f3153d95c` repair (build the historical schema once, copy per
case, all 17 upgrades kept) came in through the merge; locally it passed all 17
cases in 67.83 s. No hosted run has proved it yet: committed scratch makes
`scratch-clear` mark the PR a non-candidate, deferring the hosted suites until a
scratch-clear head (`lf pr submit`). Auto-merge on #1888 is disabled so no
recovery push merges before demo. Detail: `85b29b553e:scratch/loo-837-demo.md`.

Earlier checks (pre-retry-repair): `documented_commands::` plus the unpushed-parent
sync CLI test, 5 passed; clippy and fmt passed on `f7b316e62a`.

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

## Retry repair (2026-11-13)

`select_stack_parent` no longer retains an abandoned historical parent on retry:
it uses the existing placed-branch path to choose the live row or record an
unpublished one. Merged-parent retries still retain their identity without pushing.
`stack_existing_task` now says "This Task's checkout and PR are unchanged" after
any parent-branch publication note.

The CLI regression covers both a newer live parent record and a newly allocated
one, retaining the closed row and syncing the parent's file into the child.

- `cargo test -p loopflow --test integration sync_tests::stacking_retry_replaces_an_abandoned_parent_and_syncs`: passed both new-record and live-record cases in 8.63 s; the initial fixture assertion was corrected to snapshot history after sync lookup refreshed its observation cache.

- `cargo test -p loopflow --lib ops::task::tests::stacking_retry_keeps_a_merged_parent_without_republishing_its_branch`: passed in 0.32 s; merged identity retained with local branch present or absent, no remote publication. Both test builds emitted the existing macOS unwind-section linker warning.
- `cargo fmt --all -- --check` and `git diff --check`: passed on the retry repair; clippy remains gate-owned.

Compress: the two stacking CLI tests share one `lf(dir, args)` helper in
`sync_tests.rs`; production code unchanged.
- `cargo test -p loopflow --test integration -- sync_tests::stacking_`: 2 passed in 10.09 s.

## Remaining

Gate: clippy and scratch-clear publication so hosted CI runs the full matrix,
including the admission fixture. Affected checks include Task preparation,
checkout explanation, sync, documented commands and planning performance.
Then demo; no interactive judgment collected in this headless run.

## QA (2026-11-13, headless, local bare origin)

`scratch/qa.html`: 11 CLI scenarios with the branch `lf`. Unpushed, behind,
diverged, rejected-push, missing-branch, retry and late-stack paths behave as
documented. Fixed: the push note now reads "pushed <id>'s branch … to origin"
(it printed after the push). Scenario 11 found that re-stacking a child
onto a different parent kept the old fork as `base_commit`, so `lf sync`
deleted the old parent's files. The repair below resolves that finding.

## Re-parenting repair (2026-11-13)

Replace the old fork when an existing stack selects a different parent PR: use
HEAD's merge-base with the selected parent's upstream branch. Write parent and
fork together; identical retries retain the saved fork. Sync then treats the old
parent's inherited files as child content and merges the new parent's work.
Delete — do not maintain: the parent-only placement update when replacing a
stack dependency. Keep checkout/history, PR identity, cycle checks and retry behavior.
Focused coverage: CLI re-parenting, sync retaining both parents' files and child
edits, and an identical retry retaining the new comparison base. Gate retains
clippy and hosted suites.

- `cargo test -p loopflow --test integration -- sync_tests::stacking_`: 3 passed in 19.47 s after compress. The re-parenting CLI test now checks the fork and parent branch on select and on an identical retry, then both parents' files and child edits after sync; PR identity stays with the lib test below. The three stacking tests share `commit_file`. Production code unchanged by compress.
- `cargo test -p loopflow --lib ops::task::tests::stacking_is_idempotent_and_preserves_history`: passed in 0.47 s; parent/fork update, retry and PR identity retained. Builds emitted the existing macOS unwind-section linker warning.
- `cargo test -p loopflow --test integration documented_commands::`: 4 passed in 6.97 s.
- `cargo fmt --all -- --check`, `git diff --check`: passed; clippy and hosted CI remain gate-owned. Store API callers also reach Task initialization, handoff, PR range and Graph fixtures; their suites remain gate/CI-owned.

## Next implement direction (concept review, 2026-11-13)

Read from source at `900acf7372`; no test was run in this step.

1. **Stacking never moves a published PR's head.** `record_stack_parent` pushes
   whenever origin is behind, including a parent whose PR is open on GitHub. That
   push skips the fence every other Loopflow push shares
   (`commit.rs::push_with_upstream_if_needed`: take the PR-mutation lock, clear a
   head-pinned merge request and revoke Auto before the new head is visible).
   Counterexample: parent PR open with Auto requested, parent has commits made
   with `lf commit` and not pushed; `lf task run CHILD --stack-on PARENT` pushes
   them and GitHub may merge a head nobody gated. The Task's wording covers
   publishing a branch for a parent with no PR ("it shoudl always be safe to
   publish a remote branch"), not adding commits to one under review.
   Build: push only when the parent's record has no publication. For a published
   parent that is ahead of origin, push nothing, base on `origin/<branch>` as
   main does today, and reuse the left-out-commits note (name `lf commit -p` in
   the parent as the way to include them). Don't route this push through the
   fence: a child's command revoking the parent's Auto is a bigger surprise.
   Test: published parent with a merge request and one unpushed commit; after
   stacking, origin's branch and the merge request are unchanged and the stack
   line counts 1 commit left out. Docs (`ops.md`, `delivery.md`, `agent-api.md`,
   `--stack-on` help) say "pushed when the parent has no PR yet".
2. **The preview speaks in the future, the result in the past.**
   `checkout --explain` prints "Not run (based on already-fetched refs): pushed
   INF-124's branch …" since the QA tense fix, and
   `global_commands.rs:2737` still expects "pushes INF-124's branch parent-proof
   to origin", so that hosted test fails as written. Give the preview its own
   phrase ("stacking pushes …") and keep "pushed …" for the stack line.
3. `docs/ops.md` lists why stacking stops without "has already merged";
   `delivery.md` has it. Add it.

Not for this Task, kept as an observation: the child points at the parent's PR
row (`parent_pr_id`), though the rule people learn is "a Task with a branch".
The abandoned-parent, retry and re-parenting repairs above all come from keeping
that row in step with the Task. A child that named the parent Task and its fork
would need no record written for the parent.
