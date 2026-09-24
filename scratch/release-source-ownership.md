# Release source mutation ownership

Starting head: `bb74e99e9207b1dadd1f398608e7b49c3b5e3a74`.

## Counterexample and change

The built-CLI regression reproduced a second release tagging `v0.9.2` after
the first controller died with its initial source fetch still paused. The
contender returned `Ok("v0.9.2")`; the failing run took 4.77 seconds. This is
a concrete target-exclusion failure, not evidence about every other boundary.

Initial origin and recovery-branch fetches now use shared Git fetching with
explicit inheritance from the held target lock. Named checkout creation passes
both target and exact checkout capabilities into the existing worktree-add
implementation. Rebuild reset likewise receives both. Exact-source classification
and caller-preserving origin selection remain in place.

Ordinary removal now delegates to owned removal after acquiring its lease;
the unchecked removal helper is deleted. The one removal command inherits that
lease plus any explicit caller capabilities. Release cleanup drops the old
lease and acquires an independent description before removal, then holds that
new lease through branch deletion. Surviving earlier children still block
cleanup, while surviving cleanup children retain protection themselves. Shared
branch deletion also accepts explicit inheritance. No new lock, capability
registry, execution state, settlement writer or ambient role lookup was added.

Inspection found another mutation inside named-checkout creation: it scheduled
an asynchronous upstream push. Removed that call from the named helper, whose
production callers are release operations. Release preparation already owns an
explicit commit/push; source verification and publisher checkouts should remain
local. Ordinary placement retains its existing background synchronization. The
Git survival fixture no longer unsets an upstream implicitly created by that
removed push; its ordinary/force push cases still explicitly establish tracking.

## Proof and limits

The new CLI test crosses initial fetch, recovery branch fetch, checkout add,
rebuild reset, removal and branch deletion with controller death and failed
launcher exits. It uses disposable repositories and a real bare origin, real
CLI/Git/processes/OS locks, and shell barriers around the selected Git operation.
GitHub observations are simulated. The failed launcher starts a descendant,
then returns an error while that descendant remains paused.

Every case checks that another release defers until the child completes. Add,
reset and removal also check that ordinary removal cannot take the same checkout.
After release of the barrier, assertions inspect the actual resulting Git state:
selected checkout HEAD and reset contents, removed directory or branch, and
updated remote-tracking refs. Fetch fixtures begin with deliberately stale refs.
The remote branch set must remain unchanged by source operations. Every case
retains caller HEAD, branch, byte-identical index, staged/unstaged/untracked bytes
and an unpublished local commit.

This proves the concrete local launch boundaries and resulting mutations, not
arbitrary external programs forwarding descriptors, every materialization error
or live GitHub behavior. It supplies no installed scheduling, UI-host proof,
public artifact smoke or configured automatic settlement.

## Review and remaining work

The simulated review followed the named helper into worktree-add and its former
upstream scheduler, then followed materialization and all cleanup callers into
shared removal/deletion. Removing the hidden publication keeps release's explicit
push as its branch-publication owner. Ordinary Git entry points remain short
no-capability wrappers around one command implementation; they do not acquire
release ownership. Cleanup still checks the independent lease before deletion,
so passing capabilities into removal does not restore the prior cleanup bypass.

Task revocation/compensation remains outside this slice. Historical telemetry
prerequisite associations and once-per-wake retry, dated repair ownership,
closed-obligation continuation and configured acceptance remain open. The
observed missing `agent_turns` scorecard table and unaccepted Intelligence
handoff remain blockers. Retain all 36 observed failed telemetry targets,
including the original 35. Required UI/public proof and two adjacent automatic
executions, at least one publishing without manual repair, are unchanged.

## Validation

- New source-mutation proof: failed before inheritance (4.77s); all twelve
  cases passed after repair. Final proof with stale-ref and remote-branch-set
  assertions passed in 44.60s.
- `release_tests release_run_reintegrates_a_dirty_existing_pr`: passed, 5.54s.
- `release_tests release_run_prepares_signed_artifacts_before_pushing_the_version_tag`:
  passed, 12.46s, with simulated external services.
- `release_lock_tests surviving_release_git_mutation_retains_target_and_checkout`:
  all ten cases passed after removing the obsolete upstream-unset fixture step,
  47.57s. Ordinary and force pushes still exercise established tracking.
- `worktree_tests worktree_add_`: two ordinary caller cases passed, 0.25s.
- `release_tests release_pr_rebuild_preserves_divergent_local_branch`: passed,
  1.12s; unpublished local repair remains intact.
- `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings`: passed
  on the final source/test content (final Clippy 42.48s including build-lock wait).

No executable changes followed these final proofs. No affected-suite/full-CI gate,
installation/sync, cron trigger, production publication, PM assignment,
PR publication, landing or Task completion is part of this cut.
