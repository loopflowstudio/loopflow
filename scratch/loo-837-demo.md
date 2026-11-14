# LOO-837 — 2026-11-13

Jack Heart: "Next: merge main into the branch, read the 2 failing checks from CI and fix them directly in this checkout, push, and stop at demo."
No demo occurred: this run is headless. No reactions or approval were collected.
The stopped ship loop and personal configuration stay untouched.

## Outcome and approach

Stack on any placed Task branch. Push to origin without force when absent, or
when behind an unpublished parent. A published parent’s missing remote branch uses the commit fence:
changed heads revoke Auto before pushing; the same clean head retains intent.
Before restoring a published branch, read GitHub fresh and select the parent
again: merged refuses a new stack; closed allocates an unpublished record.
Unreadable GitHub keeps the stored reading and the existing restore behavior.
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
published-remote-absence refusal, restoration based only on a stored Open PR,
and past-tense push in preview. Preserve unpublished pushes, rejection/divergence,
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

### Missing-branch reconciliation (concept review, 2026-11-13, third pass)

Counterexample found by code review: GitHub can merge or close a published PR
and delete its branch while the store still shows Open. Restoring from that
stored reading recreated a merged branch and could disable Auto on a settled PR.
The original review and direction remain at
`8f131a0580fe66c188605b090e715f11146d6ca7:scratch/loo-837-demo.md`.

`record_stack_parent` now reconciles the Open PR with `PrReadFreshness::Fresh`
before a missing-branch restore, releases reconciliation's mutation lock, and
selects again. A merged parent refuses new stacks without pushing or disabling
Auto; an identical retry retains its historical merged identity. A closed PR
is abandoned and replaced by an unpublished parent record. Open or unreadable
GitHub restores through the existing fence. Existing remote branches stay untouched.
Preview names the execution-time GitHub read. `docs/ops.md`,
`docs/agent-api.md` and `docs/dev/delivery.md` describe Auto being turned off
for an earlier head instead of using “commit fence”; the `docs/work.md`
overview names only the push. Preview and execution share
`StackParent::has_open_pr`.

The existing missing-remote fixture now covers same-head and changed-head Open,
Merged, Closed and unreadable responses, with local Git transport and fixture
GitHub REST responses. The remote receive hook rejects an exposed changed head
while Auto is armed; merged and closed cases retain the armed marker.

Not for this Task: the child names the parent's PR row (`parent_pr_id`), though
the taught concept is a Task with a branch. Keeping the row current caused the
abandoned-parent, retry and re-parenting repairs. A parent Task plus fork could
remove the need to allocate a parent PR row; that redesign remains separate.

Earlier focused checks and preview results:
`8f131a0580fe66c188605b090e715f11146d6ca7:scratch/loo-837-demo.md`.
Only published parents take the push lock/fence; unpublished pushes need no
checkout or wait on the parent's `lf commit`.

The initial expanded test failed with “missing fresh read for open”: its fake
`gh` rejected `--version`, so reconciliation correctly treated GitHub as
unavailable. The fixture now supports that probe. An earlier compiled fixture
with duplicate environment locks was stopped; the duplicate lock was removed.

- `cargo test -p loopflow --lib ops::task::tests::task_preparation_stacks`: 5 passed (4.90 s after compress), including five missing-published-branch cases. Existing macOS unwind-section linker warning remains gate-owned.
- `cargo test -p loopflow --test integration documented_commands::`: 4 passed (7.05 s).
- `cargo fmt --all -- --check` and `git diff --check`: passed; scratch remains within both context limits. Clippy and hosted suites stay with gate.
