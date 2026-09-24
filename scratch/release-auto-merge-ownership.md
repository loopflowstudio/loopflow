# Preserve release ownership through auto-merge commands

## Remaining work

Keep the serial PR open and the Task incomplete. This cut covers shared
auto-merge enable/disable commands, initial arming and re-arming. Shared commit,
push, PR creation/editing/ready commands, notes agents, lockfile tools and
source/worktree mutation children remain separate reachable ownership gaps.
Carry the existing capabilities through those paths and prove surviving-child
behavior before claiming complete mutation exclusion.

Historical telemetry prerequisite association and bounded current recovery,
dated repair ownership, closed-obligation continuation and configured acceptance
remain open. The observed scorecard query against missing `agent_turns` is still
the named verification blocker; no Intelligence handoff was accepted here.
Original failed telemetry receipts, required UI-host/public smoke and the two
adjacent automatic settlements remain mandatory. No new independent publication
blocker or sibling Task is claimed.

## Counterexample and implementation

Starting head: `fe5f9f50c22fa28b101e896c4ca0ab91130c34e2`.
The release operation held its target lock while waiting for GitHub, but the
shared `enable_auto_merge` and `disable_auto_merge` functions constructed
ordinary commands. A GitHub mutation child could survive controller death after
the release lock had closed.

The new built-CLI regression reproduced a second release successfully tagging
`v0.9.2` while the first auto-merge child was blocked. It then proves exclusion
for both revocation and exact-head arming, through controller death and a failed
launcher leaving a live descendant. Expanded cases also reach these commands
through real release preparation, where checkout protection is required.

The existing shared auto-merge functions now accept explicit command inheritance
from their caller. Release waiting supplies its held target lock. Release PR
preparation supplies that lock and its exact checkout lease through
`finish_arm_after_rebase`, `prepare_pr` and `finalize_remote`. Replacement of an
already armed request forwards the same inheritance into revocation before
arming. The earlier non-Task revocation inside preparation also uses it.

Ordinary CLI/Task callers explicitly supply no release capability. This does not
change their process authority or create release ownership from environment.
The inheritance callback configures commands; it does not create an executor,
acquire another lock, select a role or write durable evidence. Both the target
lock and checkout lease retain their existing owners and lifetime semantics.
No alternate PR implementation, persisted field, DTO, migration, scheduler or
general process-liveness mechanism was added.

## Focused proof

The test invokes the built `lf release run` in disposable repositories with
real Git, bare origins, OS locks and process boundaries. It waits at an explicit
GitHub-command barrier, kills and reaps only its owned controller or observes
the controller fail normally, then tries a second release. After allowing the
child to finish, it observes the simulated remote arm's exact head or revocation
and verifies release access returns. Preparation cases additionally prove both
ordinary removal and controller cleanup preserve the child's checkout until
exit. GitHub and notes generation are simulated, so these are not configured
scheduled settlements, provider-agent lifetime proofs or live publication.

| Boundary | Proof | Limit |
|---|---|---|
| Re-arm after controller death | Second release defers until the surviving enable/disable child exits | Simulated GitHub mutation |
| Failed launcher | Descendant retains target exclusion after the controller returns an error | Shell descendant, not arbitrary programs closing descriptors |
| Initial preparation | Enable/disable children retain both target and checkout protection through both exit modes | Earlier commit/PR/notes subprocesses execute normally; their survival is not proved |
| Exact-head mutation | Simulated remote state records the selected head; disable removes the arm | No hosted merge or branch-policy proof |
| Normal release recovery | Existing dropped-arm and advancing-main preparation cases | Simulated remote services |
| Ordinary PR behavior | Existing ordinary arm case | No new release capability supplied |

## Source review and validation

The simulated review followed both shared mutation commands and all callers.
Replacement forwards the callback to `disable_auto_merge`; release waiting
passes its target lock; preparation passes both held locks. Ordinary CLI
recovery and Task revocation sites deliberately retain their prior behavior.
The `inherit_merge` parameter and its documentation name its limited coverage:
PR edits, readiness, commit/push and other mutations still need work. Read-only
GitHub observations do not acquire or confer mutation ownership.

The first fixture asserted the command arguments as well as exclusion. The
final fixture instead models the remote arm and asserts its resulting head or
absence, retaining the user-visible state proof. No production code was shaped
around the fixture. The complete design remains intact; only its current slice
and ledger are updated.

- `cargo test -p loopflow --test release_lock_tests surviving_release_auto_merge_child_excludes_another_release -- --nocapture`: reproduced competing tag creation before repair; final eight-case proof passed in 47.72s.
- `cargo test -p loopflow --test release_tests release_run_rearms_a_dropped_auto_merge_for_the_exact_head -- --nocapture`: passed, 11.58s.
- `cargo test -p loopflow --test release_tests release_run_reintegrates_a_dirty_existing_pr -- --nocapture`: passed, 4.67s.
- `cargo test -p loopflow --test land_tests pr_arm_publishes_without_create_flag_and_leaves_worktree_in_place -- --nocapture`: passed, 10.86s.
- `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings`: passed.

No executable change followed these final proofs. No affected-suite/full-CI gate,
install/sync, cron trigger, production publication, PM assignment, PR publication,
landing or Task completion is part of this cut.
