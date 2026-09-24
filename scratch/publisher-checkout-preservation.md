# Preserve the surviving publisher's checkout

## Remaining work

Continue the serial Task; this cut does not approve the branch or complete the
KR. Shared commit/PR operations, auto-merge re-arming, source/preparation hooks,
notes, source checkout materialization and cleanup children still need the
full release-operation exclusion proof. Historical telemetry association and
bounded recovery, dated repair ownership, closed-obligation continuation, the
remaining interruption matrix and supported configured acceptance remain open.
The observed scorecard `agent_turns` schema blocker and required UI-host/public
smoke are unchanged. No new publication blocker or sibling Task is claimed.

## Boundary and counterexample

Starting head: `8baf779945b7b388026d09f2129117454291ab30`.
`WorktreeLease` protects an exact checkout, while `ReleaseLock` protects a
repository/target's release mutation. A publisher inherited only the latter.
Killing its controller released the checkout lease even though the publisher
still held the release lock. Ordinary worktree removal could then delete the
source under the surviving child.

The built-CLI regression reproduced this: after the exact fixture controller
was killed and reaped, `worktree_remove` succeeded on the paused preparation
child's checkout. The test failed with `removed surviving prepare child's
checkout`. The fixture's initial missing `.lf` directory was repaired before
this behavioral reproduction; it was not counted as the product failure.

## Implementation and ownership

`WorktreeLease::inherit` passes the already-owned OS descriptor to the selected
child, using `pre_exec` only to clear that child's close-on-exec flag. The lease
file, checkout identity, acquisition and ordinary removal authority are unchanged.
No descriptor is interpreted as permission to bypass checkout acquisition.

Preparation, publication and public reconciliation borrow their existing lease
into the publisher command alongside the existing target lock. The publisher,
website deployment and release-packaging Python helpers forward both descriptors
to their subprocesses. The publisher entry point continues to require the
release descriptor; a checkout descriptor alone cannot authorize a publisher
stage. This preserves the two owners rather than inventing a combined lease,
ambient execution role, durable liveness record, or second cleanup mechanism.

No settlement format, scheduled prerequisite policy, cron timing, caller source
selection or historical receipt changes. The complete design remains intact;
only its current slice and ledger were updated.

## Focused proof

| Boundary | Observation | Limit |
|---|---|---|
| Preparation controller dies | Ordinary checkout removal fails; release contender defers; child reads unchanged source bytes; cleanup succeeds after exit | Real CLI/Git/worktree/OS locks, simulated hosted workflow and publisher |
| Publication controller dies | Same preservation and recovery as preparation | Same local limits; no actual signing/upload |
| Checkout independence | Another checkout in the same repository is removed while publisher protection remains held | Local filesystem proof |
| Python descendants outlive launchers | Both OS locks remain exclusive after helper/uv exit and original handles close; both become available after descendant exit | Real subprocesses through publisher, deployment, packaging `run` and `run_capture`; no external services |
| Distinct capabilities | Checkout descriptor without release descriptor cannot enter verify or reconcile | Focused publisher entry-point rejection |
| Normal candidate path | Existing candidate preparation, tag and publication flow completes with child-held checkout leases | Existing integration uses simulated services |
| Public reconciliation | Same explicit lease inheritance at command launch | Source inspection; no separate reconciliation kill point in this cut |

The simulated review retained the existing lease/removal boundary and rejected
making removal consult release state: other checkouts and ordinary non-release
operations must remain independent. It also caught that forwarding two fds
must not let a nonempty checkout-only list satisfy the publisher's release-lock
requirement. The explicit release requirement and its regression preserve that
boundary.

## Validation

- `cargo test -p loopflow --test release_lock_tests surviving_publisher_keeps_its_checkout_after_controller_death -- --nocapture` — passed both stage scenarios after the reproduced failure; final execution including independent checkout removal: 7.66s.
- `cargo test -p loopflow --test release_tests release_run_prepares_signed_artifacts_before_pushing_the_version_tag -- --nocapture` — passed, 12.38s execution.
- `uv run pytest python/tests/test_release_publisher.py -q -k 'descendant_retains or direct_publisher_stage'` — six passed, 17 deselected, 0.36s.
- `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, and Ruff check/format on the four changed Python files — passed.

Only formatting, equivalent adjacent string-literal splits and documentation
changed after the Python proof. No affected-suite or full-repository gate was
run. Fixtures cannot supply the two configured automatic settlements, required
UI-host gate, or public exact-tag smoke. No install/sync, cron trigger, production
release, PM assignment, PR publication, landing or Task completion occurred.
