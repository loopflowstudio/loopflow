# LOO-692 demo: landing the lower PR of a stack (2026-10-30, headless)

Experience reviewed: `lf wt prune --branches` (the shared `wt::apply_delete`
path that land, completed-Task cleanup and abandon also use) run twice over a
stack where the lower PR is merged and the upper PR is open on the lower branch.

Path: the built `lf` binary against a local bare origin with a stateful `gh`
stand-in (`landing_the_lower_of_a_stack_keeps_the_upper_pr_open_on_its_branch`,
`rust/loopflow/tests/wt_prune_branches.rs`). The stand-in closes a PR when its
base branch is deleted, as GitHub does, so the second sweep reproduces the
manabot 13:32:42Z → 13:33:00Z sequence.

Observed: `cargo test -p loopflow --test integration wt_prune_branches::` — 9 passed.
After two sweeps the upper PR is OPEN with base `main`, origin heads are
`main, upper`, and the upper branch keeps its commit. The implement pass
recorded that disabling the retarget step makes the test fail (CLOSED ≠ OPEN).

Not demonstrated: a live GitHub stack. Creating and merging real PRs is
outward-facing and no one was available to authorize it; `verify` after merge
is the place to watch the next real stacked landing.

Feedback: none (headless; the agent stood in for Jack at the demo decision).
Decision: advance to gate. Open questions stay in `scratch/questions.md`
(abandon retargets rather than keeping the base; hand-closed PRs keep their branch).
