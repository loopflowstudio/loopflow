# Interrupted telemetry recovery

Starting head: `c26e16f95773dd5a71893b3f077456a8a74101c9`.

A retained Running telemetry receipt previously deferred every later release
wake, even after its runner died and its child exited. The built-CLI regression
reproduced that boundary: after killing and reaping its own cron controller,
the second firing still stopped before the executor's surviving-child fence
(12.80s). No production process was interrupted.

New physical receipts retain optional `runner_started_at`, captured from the
existing OS process-start probe alongside `runner_pid`. The journal's existing
PID/start comparison is shared with cron; Task evidence still first requires its
own registered Exec receipt or terminal event. The comparison retains the
existing three-second observation tolerance. A live or unknown runner defers.
Confirmed runner death permits one reserved recovery through the same installed
executor, which must acquire the existing job lock before launch. A surviving
child keeps that lock. No PID age, timeout, or missing registry grants authority.

The interrupted physical receipt remains Running with no fabricated exit code
or completion time. Current verification references the new successful Recovery
receipt. Reservation, frozen coverage, placement checks, doctor continuity and
product settlement keep their existing owners. Schema-1 historical receipts
without the optional field still read as unknown; the history fixture and Rust
constructors were updated. Searches found no Swift or Python receipt mirror.

## Behavioral proof

The new interruption scenario uses the built cron CLI and release Flow in a
registered disposable Home. Three firings prove:

1. A live runner defers without reserving a retry or publishing.
2. After exact controller death, the surviving check excludes execution. The
   one reserved recovery receipt records the overlap; the check is not relaunched.
3. After the child finishes and releases its job lock, the next firing performs
   one successful recovery and settles publication with that receipt's proof.

The fixture retains the original Running receipt, all three attempts, collapsed
coverage and exact caller HEAD/branch/index/staged/unstaged/untracked bytes.
A second CLI case reads an old receipt with no start identity and a missing PID;
it stays deferred without recovery or publication. The focused identity case
also distinguishes the current process from an older identity using the same PID.

Git, bare origins, CLI, registry, processes and OS locks are real. Telemetry is
a shell check; GitHub and publisher proof are simulated. Due dates are seeded.
These are local recovery proofs, not real launchd firings, configured telemetry,
hosted builds, UI-host verification, public artifact smoke or either required
qualifying automatic settlement.

## Review and remaining work

The simulated review kept runner death separate from child exit and physical
completion separate from current recovery. It rejected deriving authority from
the existing six-hour stale-display hint. The original receipt is never rewritten,
and the inherited job lock remains the final execution exclusion. No new worker,
liveness registry, lock owner or settlement writer was introduced. Arbitrary
programs that discard inherited descriptors are outside this local shell proof.

The executor still waits synchronously for its target. Bounding that wait remains
open, as do the actual telemetry Flow's missing-natural-receipt continuity boundary,
previous telemetry obligation segments, dated repair ownership, closed-obligation
continuation, remaining interruption proofs and configured acceptance. The
36 failed telemetry targets, missing `agent_turns` scorecard blocker, unaccepted
Intelligence handoff, required UI/public proof and two adjacent automatic
settlements remain unchanged. No install/sync, trigger, publication, PM handoff,
PR publication, landing or Task completion occurred.

## Validation

Final results are recorded below after completion. No affected-suite or full
repository gate is claimed.

- Initial interruption regression: failed before repair (12.80s); passed after
  repair (18.01s). The legacy unknown-identity CLI case passed (7.62s).
- Final `cargo test -p loopflow --test scheduled_release_tests -- --nocapture`:
  three tests / ten scenarios passed, 85.50s. This includes the added overlap
  receipt and recovered verification-subject assertions, plus the existing eight
  settlement/recovery/preservation scenarios.
- Four focused library cases passed: new cron runner identity/legacy/PID-reuse
  case; existing exact live/terminal Exec evidence; existing killed registered
  Exec evidence; release-history DTO/failure-disposition fixture.
- `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` (19.55s)
  and `git diff --check`: passed.
