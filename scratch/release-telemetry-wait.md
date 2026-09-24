# Bounded telemetry recovery observation

Starting head: `1acf91f703be017d5fe3cf5e3c6f9ed68f0e8e6d`.

## Change

Automatic prerequisite recovery previously waited indefinitely in
`Command::status()`. It now uses the same cron launcher and waits at most one
hour, matching existing release-stage wait policy. Ordinary scheduled/manual
cron targets retain their blocking wait. No configuration knob, executor,
background receipt writer or scheduler was added.

The cron executor separates spawn from observation. A spawn failure still
writes a terminal Failed receipt. Only an observed exit sets `finished_at`,
`exit_code` and the terminal outcome. Deadline expiry returns typed Deferred.
An OS observation error returns a command failure: unavailable exit evidence is
not an intentional wait. Both leave the reserved Running receipt intact.
The release owner already persists that deferral and appends the exact next
configured release due time. The message identifies the receipt and log path.

The child retains the existing job descriptor when the controller stops waiting.
No process is killed and no age/PID inference grants another launch. A later
wake must still acquire the job lock before starting its one reserved recovery.
An unobserved late exit does not retroactively pass the old receipt. Fresh
successful recovery is new current proof. This bounds release's wait, not the
check's runtime; an indefinitely running check remains excluded and unresolved.
The normal CLI exits after reporting deferral; there is no detached observer to
write a late result or another durable execution owner.

## Focused proof

The new library test uses the production launcher, wait function, physical
receipt store, installed executor and real OS job lock in a disposable Home.
A 50ms observation window returns Deferred while a real shell check waits on a
barrier. Dropping the controller's lock still leaves a contender excluded.
After releasing the check, its late successful exit leaves its original receipt
unchanged; a fresh executor invocation produces a distinct successful receipt.
The test checks actual start/completion effects and durable receipt identity.

The short deadline exercises the production wait function directly. It does
not claim a one-hour wall-clock CLI timeout demonstration or persist a release
attempt at that timeout. The existing built-CLI interruption test covers the
joined executor/Flow/release/history path, including live-runner deferral,
surviving-child exclusion after controller death and later successful recovery.
Its telemetry shell, GitHub and publisher proof remain simulated; due times are
seeded. Neither proof is configured acceptance or the actual doctor/scorecard
Flow.

## Review

Simulated review kept the deadline outside process authority. Polling observes
only this owned child's exit; it never signals it or substitutes elapsed time
for an exit status. The existing terminal writer remains in `run_cron_recorded`.
Review changed observation errors from Deferred to command failure, preserving
the design's distinction between bounded waiting and unavailable authority. A
focused real-process test reaps its own child outside the observer to reproduce
that error; the receipt retains its unknown exit. Separating launch errors from
observation errors also avoids falsely claiming the target never started. No receipt field, DTO,
new persisted status or compatibility decoder was needed.

The terminal-write refactor is checked with ordinary successful and failed
telemetry paths as well as the deadline boundary. Cron attribution, job
exclusion, release target ownership and checkout protection remain distinct.
Original failure evidence, frozen coverage and the once-per-wake reservation
policy are unchanged.

Actual telemetry continuity, prior telemetry obligation segments, dated repair
ownership, closed release continuation and remaining interruption proof stay in
LOO-285. The retained 36 failed checks, missing `agent_turns` scorecard blocker,
unaccepted Intelligence handoff, required UI/public proof and two adjacent
automatic settlements remain open. No installation, schedule change, production
publication, PM write, PR publication or Task completion occurred.

## Validation

- `cargo test -p loopflow --lib ops::cron::tests::telemetry_ -- --nocapture`:
  three final deadline, observation-error and job-exclusion cases passed (0.60s).
- `cargo test -p loopflow --lib runner_persists_success_and_failure_receipts -- --nocapture`:
  ordinary success/failure receipt proof passed (0.21s).
- `cargo test -p loopflow --test scheduled_release_tests interrupted_telemetry_recovers_only_after_its_runner_and_child_exit -- --nocapture`:
  built-CLI proof passed, three firing boundaries (36.96s).
- An initial broader `--lib telemetry_` name filter passed ten tests (7.96s),
  including reservation fencing and the first deadline case. Final validation
  narrowed to the three changed cron boundaries above.
- The CLI and ordinary receipt passes precede the final observation-error
  classification repair. That error branch has its own final focused proof;
  normal waiting and terminal receipt paths did not change afterward.
- `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and
  `git diff --check`: passed. Final Clippy finished in 35.68s including build-lock
  wait. Only documentation changed after final code validation.

No affected-suite gate, full CI, one-hour wall-clock timeout, actual telemetry
Flow, UI-host or public-artifact verification was run.
