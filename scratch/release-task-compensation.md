# Task compensation retains release ownership

## Change and authority

The release target lock and exact checkout lease now reach Task-owned remote
revocation through the existing command inheritance callback. Shared commit
passes it through the pre-push settlement fence. Shared PR finalization passes
it through initial clearing, stale-head attachment, merge-request replacement
and failed-finalization compensation. Ordinary restart, rebase, publication,
Task resume and status observation supply no release capability.

Task resolution, exact-head validation and mutation ordering are unchanged.
Remote revocation must return successfully before durable merge intent is
cleared or replaced. A failed revocation leaves that request intact. No new
lock, executor, state object, ambient authority lookup or settlement writer was
introduced. The Task PR mutation guard retains its existing scope; this change
extends release target/checkout protection, not all Task process lifetimes.

## Counterexample and proof

The built-CLI regression registers the actual generated release branch as a
Task in a disposable Home while notes preparation is paused. It supplies a valid
stored Auto request for the prior head. This deliberately exercises branch-based
Task resolution; clearing ambient Run/Wave variables does not make that branch
unmanaged. The release then commits its prepared notes and reaches the shared
Task fence before pushing.

Before the fix, killing the controller while its `gh` revocation child waited
allowed a competing release to return `Ok("v0.9.2")` (8.07s). The test therefore
reproduced mutation access, not merely missing callback wiring.

After the fix, both controller death and failed-launcher/surviving-descendant
cases pass. While the child waits, another release defers, ordinary checkout
removal fails, the checkout remains usable, the original durable merge request
is unchanged and the bare origin has no new release branch. Once released, the
simulated remote records disabled auto-merge; the interrupted controller has not
invented local completion. Ordinary commit/push replay observes that remote
state, clears the durable request and pushes the exact prepared head. Caller
HEAD, branch, raw index, staged/unstaged and untracked work remain preserved.

Git, bare origin, CLI, Task store, processes and OS locks are real. GitHub and
notes generation are simulated. Registering a fixture Task is not an operational
Task placement claim. The survival proof exercises the shared pre-push fence;
other compensation routes have source propagation and ordinary behavioral
checks, not separate killed-child demonstrations. No live release or scheduled
settlement is proved.

## Review

The simulated review checked each Task revocation call and its callers. All
three remote revocation owners now use the supplied callback. Ordinary
observation/resume remain explicit no-op callers; the release path does not gain
Task authority from possessing either lock. Failures still retain exact-head
intent, and replay tests inspect durable and remote state. The callback changes
neither opportunity accounting nor verification/settlement policy.

The new fixture originally exposed invalid test setup (missing reviewer copy,
missing simulated GraphQL authority, and timestamp precision on a pre-persistence
value). Those were corrected before the behavioral red result above. No
production change was based on those setup errors.

## Remaining Task work

Historical telemetry prerequisites and bounded recovery, dated repair ownership,
closed-obligation continuation, remaining interruption proof and configured
acceptance remain open. Retain all 36 observed telemetry failures, the missing
`agent_turns` scorecard blocker and the unaccepted Intelligence handoff. Required
UI-host/public exact-tag proof and two adjacent automatic executions, at least
one publishing without manual repair, still govern completion. The serial PR
remains unpublished and the Task incomplete.

## Validation

- `cargo test -p loopflow --test release_lock_tests surviving_task_revocation_retains_release_ownership_and_settlement_intent -- --nocapture`: passed both interruption cases and replay, 12.33s.
- `cargo test -p loopflow --test pr_tests revokes -- --nocapture`: three passed, 9.12s (changed-head observation, human resume and revocation before push).
- `cargo test -p loopflow --test land_tests land_clears_the_durable_request_when_auto_arm_fails -- --nocapture`: passed, 11.25s.
- `cargo test -p loopflow --test land_tests latest_land_disposition_wins_before_merge -- --nocapture`: passed, 23.02s.
- `cargo test -p loopflow --test release_tests release_run_prepares_signed_artifacts_before_pushing_the_version_tag -- --nocapture`: passed, 13.11s.
- `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `git diff --check`: passed. Clippy took 52.56s including build-lock wait.

No affected-suite gate, full CI, installation/sync, cron trigger, production
publication, PM handoff, PR publication, landing or Task completion occurred.
