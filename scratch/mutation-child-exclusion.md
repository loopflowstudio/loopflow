# Direct release mutation children

## Remaining work

This cut closes the direct tag/candidate/publication command gap. It does not
close the full exclusion obligation or approve the branch. Continue in this Task:

- Carry exclusion through shared commit/PR operations, including auto-merge
  re-arming, and through hooks, release notes, source checkout mutation, and
  cleanup. Preserve the existing generated-worktree stage ownership too. A
  target lock alone does not prove that a separate `wt remove` honors a stage
  after its parent dies.
- Retain every covered original telemetry prerequisite and current recovery
  linkage; resolve bounded prerequisite retry and repair ownership. The observed
  missing `agent_turns` scorecard table remains a verification blocker.
- Give closed unfinished obligation segments explicit continuation/disposition.
- Finish the interruption matrix and supported installed acceptance: required
  UI-host/public smoke and two adjacent automatic executions with at least one
  publication. No fixture here qualifies for that pair.

## Change and counterexample

Starting head: `898989b95eea5872928bb9dce83ad196e7177626`.
The existing release lock survived explicit publisher launches, but direct
`git`/`gh` commands closed its descriptor on exec. Killing the controller could
therefore release target exclusion while its mutation child was still running.

Both new regressions reproduced that race. The publication contender returned
`Ok(())`; the tag contender returned `Ok("v0.9.2")` while the first child was
paused before its mutation. The tag fixture initially lost its output pipe when
the controller died; redirecting the real push's output to a retained fixture
log allowed the intended surviving-child counterexample to execute.

`ReleaseLock::command` now builds a child with the existing exact descriptor
inheritance. The owning lock is borrowed explicitly through tag creation/push,
candidate-ref creation/deletion, missing-object fetch, candidate workflow
submission, and GitHub draft/edit/upload/finalize. Existing publisher
prepare/publish/reconcile launches use that same constructor. No global descriptor
inheritance, ambient role inference, independent lock owner, or liveness ledger
was added. Read commands retain their ordinary construction. The redundant
`tag_and_push` forwarding helper was removed.

## Focused proof

The new integration tests start the built `lf`, wait until its mutation child
reaches an explicit barrier, kill and reap that exact owned controller, and try
a second release operation. They then release the child and observe its effect
before proving the target can resume. Deadline polling bounds fixture waits;
only OS lock acquisition grants the contender access.

| Boundary | Observed result | Limit |
| --- | --- | --- |
| Tag push after parent death | Same-target contender defers; surviving child pushes only v0.9.1; same-tag resume succeeds after exit | Real Git/bare origin, disposable repository; shell barrier wraps the real push |
| Repository independence | A second repository tags successfully while the first child retains its lock | Local fixture |
| Publication after parent death | Contender defers; only the first child records a publication; target becomes usable after exit | Real CLI/process boundary, simulated GitHub side effect |
| Candidate preparation and completion | Existing signed-artifact-before-tag integration passes through candidate ref, workflow submission, tag, publisher, and ref cleanup | External services and signing simulated |
| Missing candidate object | Exact object fetched from another clone remains available under the borrowed lock | Real temporary Git repositories |

The simulated code review followed all direct `git push` and GitHub publication
writers in `release.rs`. Each covered mutation now uses the existing lock's
command constructor. `finish_candidate` supplies the same borrowed owner for
manual and scheduled execution; settlement and opportunity storage are unchanged.
Shared PR writers and other subprocess paths named above remain reachable and
are explicitly outside this cut's proof. They still block full-design acceptance.

## Validation

- `cargo test -p loopflow --test release_lock_tests -- --nocapture`: both
  parent-death cases passed after reproducing their failures before the repair.
- `cargo test -p loopflow --test release_tests release_run_prepares_signed_artifacts_before_pushing_the_version_tag -- --nocapture`: passed (14.46 seconds execution).
- `cargo test -p loopflow --lib ensure_commit_local_fetches_object_pushed_by_another_clone`: passed (0.46 seconds execution).
- `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings`: passed.

No affected-suite gate, full CI, installed cutover, cron trigger, production
publication, PM write, or repair handoff was performed. The earlier iterate
verdict remains applicable; the pinned lifecycle retains landing and completion.
