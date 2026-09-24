# Preserve configured release hooks through controller exit

## Remaining work

Keep the serial PR open for implementation. Shared commit/PR/re-arm operations,
release-note agents, manifest lockfile tools, source/worktree creation and cleanup
subprocesses still need complete surviving-child exclusion. Retain historical
telemetry prerequisite associations and bounded recovery, dated repair ownership,
closed-obligation continuation, and the remaining interruption matrix. Supported
installation, required UI-host/public smoke and two adjacent automatic executions
with at least one publication remain unproven. The observed `agent_turns`
scorecard blocker and unaccepted Intelligence handoff remain unchanged.

## Reproduced boundaries and implementation

Starting head: `4f381e09381714cdadccbd61c8cf2bfe6a5d05b3`. This cut follows the
publisher checkout review into configured repository verification/preparation
hooks. The built CLI fixture first reproduced a contender successfully tagging
`v0.9.2` while the original verification hook survived its killed controller.

The hook runner now borrows the existing target lock and exact checkout lease.
Its shell inherits both descriptors. Verification already owned a checkout;
release PR preparation and rebuild now acquire that same existing lease before
materializing their checkout. All cleanup callers transfer a required lease,
which cleanup drops before independent reacquisition. A surviving descendant
therefore keeps ordinary removal and controller cleanup excluded. No new owner,
lock type, persistent state, ambient role inference or parallel hook runner was
introduced. Preparation consumes the existing `ReleaseChangeSet` instead of
splitting its fields, and the single-caller rebuild helper was inlined beside
its lease owner.

The preparation case then reproduced the same successful contender despite
explicit descriptor inheritance. Its `create_named_worktree(..., true)` called
`sync_main` internally. In the disposable repository, synchronization stashed
untracked state including the held lock file and restored a different inode.
The surviving child retained the old file, while a contender acquired the new
one. This also exposed a caller reset path that earlier negative source reviews
had missed. Those earlier “no reachable sync_main” claims were too broad.

Both release PR worktree creation calls now disable that implicit sync. Initial
preparation uses the already selected immutable origin commit. Rebuild explicitly
fetches the default branch under the target lock, then resets only its owned
checkout. Thus upstream refresh remains while caller HEAD/index/working bytes
and the held lock inode stay intact. Exact-source publisher/verification creation
already disabled implicit sync. This repairs the accepted preservation design;
it does not alter settlement or prerequisite policy.

## Focused proof

The new `surviving_release_hook_retains_target_and_checkout_ownership` fixture
runs four real CLI cases: verification/preparation crossed with controller death
or failed launcher leaving a descendant. Each blocks at an explicit hook barrier,
then checks that a second release defers, ordinary removal fails, source bytes
remain usable, and target access/removal return after child exit. Each also
retains an unpublished caller commit, branch, byte-identical index, staged and
unstaged file contents, and an untracked file. Real Git, bare origin, processes
and OS locks execute; GitHub is simulated. These are not configured scheduled
settlements or hosted/UI/publication proofs.

The review follows the hook call graph and its cleanup owner. Repository hooks
are covered; subsequent notes, PR commands and worktree-mutating subprocesses
are still separate reachable gaps. Python/custom programs may close inherited
descriptors themselves; the fixture exercises shells and their descendants, not
an assertion about every arbitrary external program. The existing publisher
helpers have their separately recorded forwarding proof.

## Validation

- `cargo test -p loopflow --test release_lock_tests surviving_release_hook_retains_target_and_checkout_ownership -- --nocapture`: failed before repair for verification, then preparation; all four cases passed after removing implicit sync. Final caller-preservation execution: 14.01s.
- `cargo test -p loopflow --test release_tests release_run_reintegrates_a_dirty_existing_pr -- --nocapture`: passed, 3.55s; preparation reruns after upstream advances.
- `cargo test -p loopflow --lib release_hook -- --nocapture`: two passed, expansion and failure propagation.
- `cargo test -p loopflow --lib the_release_run_canonicalizes_drafts_into_the_committed_tree -- --nocapture`: passed.
- `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings`: passed.

The final hook proof preceded removing the now-unnecessary `Option` around the
cleanup lease; rebuild and library proofs compiled that equivalent type
reduction. No behavioral change followed the hook proof. The complete Task diff
was read back without truncation; only this slice's source, test, README and
scratch artifacts changed. Searches find all three release worktree creation
calls disable implicit sync, one hook runner, no restored duplicate success-proof
wrappers or verification writer, and no new storage or execution service.

No full-suite gate,
installation, cron trigger, production release, PM assignment, PR publication,
landing or Task completion is part of this cut.
