# LOO-837 — 2026-11-13

Jack Heart: "Next: merge main into the branch, read the 2 failing checks from CI and fix them directly in this checkout, push, and stop at demo."
No demo occurred: this run is headless. No reactions or approval were collected.
The stopped ship loop and personal configuration stay untouched.

## Outcome and approach

Stack on any placed Task branch. Push to origin without force when absent or
behind only when the parent has no publication. Record an unpublished Task PR
when needed; open no parent GitHub PR. Share parent selection between new and
existing placements. Origin remains the child's base; the stack message counts
omitted local commits. Placement without a Task PR is intentional; stacking or
publication records it when needed.

An abandoned parent PR is historical: use the live record or allocate an
unpublished one from the placed branch. A merged parent still refuses new stacks:
its pre-squash commits and sync's main path cannot distinguish new post-merge
work. An identical retry retains its merged parent identity without republishing.
A changed parent replaces both parent and fork with HEAD's merge-base against
that parent's upstream branch. Sync keeps the old parent's files as child work.

## Delete — do not maintain

Separate existing/new stack selectors and published-parent preconditions were
replaced by `select_stack_parent` / `record_stack_parent`. Keep that one path.
Remove the abandoned-specific refusal, historical abandoned-parent retry,
parent-only re-parenting update, unconditional published-parent push, and
past-tense push in preview. Preserve unpublished pushes, rejection/divergence,
history, cycle checks, retry identity, child edits and squash-parent sync.

## Recovery and earlier checks

Details and exact check results: `735070a0e0:scratch/loo-837-demo.md`.
CI run 43033634214 timed out the admission migration fixture (120.012 s).
Main's `3f3153d95c` repair (build historical schema once, copy per case, retain all
17 upgrades) was merged; the local 17-case run passed in 67.83 s. Hosted suites
have not proved it: committed scratch marks the PR a non-candidate until
scratch-clear publication. Auto-merge on #1888 remains disabled before demo.

Local CLI QA (`scratch/qa.html`) covered 11 scenarios. Re-parenting initially
kept the old fork, and sync deleted the old parent's files; the repaired path
passed the three stacking CLI regressions in 19.47 s. The history/idempotence
library test passed in 0.47 s; documented commands passed four tests in 6.97 s.
Those earlier results do not cover this iteration's published-parent repair.

## Remaining

Gate owns clippy,
scratch-clear publication, and hosted suites including admission, Task preparation,
checkout explanation, sync, documented commands and planning performance.
Then demo; no interactive judgment collected here. Store API callers also reach
Task initialization, handoff, PR range and Graph fixtures.

## Published parent repair (concept review, 2026-11-13)

The reviewed counterexample: a published parent has Auto requested and a local
commit made with `lf commit`; stacking a child must not push that unreviewed head.
The original direction was “it shoudl always be safe to publish a remote branch”.
The headless assumption in `questions.md` limits that to unpublished parents;
a child's command must not revoke its parent's Auto through the commit fence.

`stack_parent_branch` now selects a push only for an unpublished parent.
Published parents with local-only commits share the existing omitted-commit
report and base on origin; a missing published remote branch names the absence
and points to `lf commit -p` in the parent checkout. `StackParentBranch::note`
takes the push verb: checkout explanation passes “stacking pushes”, execution
“pushed”. `StackParent::is_published` is the one published-parent test. Docs and help describe the same rule,
and `ops.md` names the already-merged refusal.

Delete — do not maintain: the unconditional published-parent push decision,
`StackParentBranch::Diverged` (now `LeftOut`, covering published parents too),
and past-tense push wording in checkout preview. Keep one selector and recorder,
unpublished publication, divergence, retries, history and re-parenting behavior.

Regression: a published parent with Auto requested and one unpushed commit;
stack preparation uses the published base, origin and local heads remain intact,
the full parent PR record (including its merge request) is unchanged, and the
stack note counts one omitted commit with the parent push command.

Not for this Task: the child names the parent's PR row (`parent_pr_id`), though
the taught concept is a Task with a branch. Keeping the row current caused the
abandoned-parent, retry and re-parenting repairs. A parent Task plus fork could
remove the need to allocate a parent PR row; that redesign remains separate.

- `cargo test -p loopflow --lib ops::task::tests::task_preparation_stacks`: three existing cases passed; the new Auto fixture required PR presentation copy. After correcting the fixture, `...::task_preparation_stacks_without_moving_a_published_parents_merge_request` passed (3.76 s). Build emitted the existing macOS unwind-section linker warning; clippy remains gate-owned.
- `cargo nextest run -p loopflow --test global_commands --test integration -E 'test(task_checkout_explain_reuses_explicit_and_inferred_work_without_effects) | test(documented_commands::)'`: 5 passed; checkout preview took 73.753 s, no effects on the checkpointed store or Git refs.
- `cargo fmt --all -- --check` and `git diff --check`: passed; scratch remains within both context limits.
- Compress (shared note, `is_published`): `cargo test -p loopflow --lib ops::task::tests::task_preparation_stacks` 4 passed (6.31 s); `cargo nextest run -p loopflow --test global_commands -E 'test(task_checkout_explain_reuses_explicit_and_inferred_work_without_effects)'` passed (75.128 s). Clippy remains gate-owned.
