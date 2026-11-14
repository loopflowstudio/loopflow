# LOO-837 — 2026-11-13

Jack Heart: "Next: merge main into the branch, read the 2 failing checks from CI and fix them directly in this checkout, push, and stop at demo."
No demo occurred: this run is headless. No reactions or approval were collected.
The stopped ship loop and personal configuration stay untouched.

## Outcome and approach

Stack on any placed Task branch. Push to origin without force when absent, or
when behind an unpublished parent. A published parent’s missing remote branch uses the commit fence:
changed heads revoke Auto before pushing; the same clean head retains intent.
Record an unpublished Task PR when needed; open no parent GitHub PR. Share parent selection between new and
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
parent-only re-parenting update, unconditional published-parent push,
published-remote-absence refusal, and past-tense push in preview. Preserve unpublished pushes, rejection/divergence,
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
The published-parent repair has its own checks at the end of this file.

## Remaining

Gate owns clippy,
scratch-clear publication, and hosted suites including admission, Task preparation,
checkout explanation, sync, documented commands and planning performance.
Then demo; no interactive judgment collected here. Store API callers also reach
Task initialization, handoff, PR range and Graph fixtures. Realign
(2026-11-13) found code, help and the four docs agreeing with the
published-parent rule. Missing-remote restoration is implemented and its
regression passed (below); realign (2026-11-13, second pass) found code, docs
and scratch agreeing on it.

## Published parent repair (concept review, 2026-11-13)

The reviewed counterexample: a published parent has Auto requested and a local
commit made with `lf commit`; stacking onto its existing remote branch must not
push that unreviewed head.
The original direction was “it shoudl always be safe to publish a remote branch”.
The earlier headless assumption in `questions.md` was superseded by iteration
direction: “Implement publication for that case while respecting the parent’s
commit-fence behavior”. Existing published remote branches stay untouched.
Missing branches are pushed through the existing parent mutation lock and
`clear_task_pr_merge` fence; no GitHub PR is opened. Preview and execution share
`stack_parent_branch`; the missing-remote refusal is deleted. Docs and help now
describe restoration and Auto revocation.

Regression: a published parent with Auto requested and one unpushed commit;
stack preparation uses the published base, origin and local heads remain intact,
the full parent PR record (including its merge request) is unchanged, and the
stack note counts one omitted commit with the parent push command.

### Next implement direction (concept review, 2026-11-13, third pass)

Counterexample, read from the code (not run): GitHub closes a PR when its head
branch is deleted, and squash landing deletes the branch. So a parent the store
still shows Open (GitHub number recorded) whose `origin/<branch>` is gone after
`fetch --prune` has merged or closed on GitHub; the store has not read it yet
(Auto landed between observations). `record_stack_parent` today trusts the
store: it pushes the merged branch back to origin, and on a changed head calls
`disable_auto_merge` on a PR that can no longer merge. That bypasses two rules
above: a merged parent refuses, a closed PR falls back to the placed branch.
The fixture `gh` never reports the PR closed, so the regression passes.

1. In `record_stack_parent`, when the selected parent's PR is Open and origin
   lacks its branch, read the PR fresh first (`reconcile_task_pr_observation`,
   `PrReadFreshness::Fresh`; it takes and releases the parent mutation lock
   itself) and select again. Merged then refuses; closed stacks as an
   unpublished parent with a new record. Still Open, or GitHub unreadable:
   restore through the fence as now, on the store's reading. No new refusal.
   The fence stays, per “Implement publication for that case while respecting
   the parent’s commit-fence behavior”.
2. Regression: fixture `gh` reports the PR merged, remote branch absent, store
   Open. Stacking refuses with the merged message; origin gains no branch and
   `--disable-auto` is never called. Same for closed: branch pushed, a new
   unpublished parent record, the old one abandoned.
3. “Commit fence” is internal; no doc on main uses it. In `docs/agent-api.md`,
   `docs/dev/delivery.md`, `docs/ops.md` and `docs/work.md`, say what happens:
   “If Auto was requested for an earlier head, stacking turns Auto off before
   pushing.” Add that a parent whose PR merged or closed on GitHub is read
   first. The preview line may say the PR is read from GitHub at execution.

Not for this Task: the child names the parent's PR row (`parent_pr_id`), though
the taught concept is a Task with a branch. Keeping the row current caused the
abandoned-parent, retry and re-parenting repairs. A parent Task plus fork could
remove the need to allocate a parent PR row; that redesign remains separate.

Earlier focused checks and preview results: `b89812f9fc:scratch/loo-837-demo.md`.
The missing-remote regression covers unchanged and changed heads, checking origin,
child base, parent PR identity/copy, retained same-head Auto and remote revocation
before a changed-head push.

- `cargo test -p loopflow --lib ops::task::tests::task_preparation_stacks`: 5 passed (3.51 s); the new missing-remote case with its final environment lock passed separately (2.09 s). Existing macOS unwind-section linker warning remains gate-owned.
- Compress: only a published parent takes the parent lock and fence, so an unpublished parent needs no checkout and its push never waits on the parent's `lf commit`; the post-clear PR re-read is gone (callers read id and branch). Same filter: 5 passed (4.61 s).
- `cargo test -p loopflow --test integration documented_commands::`: 4 passed (6.94 s).
- `cargo fmt --all -- --check` and `git diff --check`: passed; scratch is within both context limits.
