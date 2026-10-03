# Scheduled overlap continuation

Scheduled target-lock contention now records the exact next configured release
due time, obligation and original Home. Cron job overlap uses the same calendar
calculation. This advances LOO-285's continuation contract; closed-segment
execution recovery and configured acceptance remain open.

## Ownership and behavior

The existing obligation supplies the schedule and timezone. At contention, the
reader resolves the exact current receipt owner and calculates the first due
strictly after observation using the shared calendar. It does not reuse the
opportunity's saved next due, which may already be past after a long execution.
The existing release error path writes the resulting Deferred outcome and its
verification evidence through `settle`. CLI error output contains the same
continuation. Manual release calls retain their existing behavior.

The job-overlap writer already holds its accounting lock. It uses the same
calculation when recording a new deferred opportunity, without changing the
active attempt or its frozen covered keys. Its existing reason retains the
known active receipt. No new lock metadata, reservation, timer, executor, DTO or
durable writer was added.

If the retained segment is closed, the continuation names its closure, original
Home and exact opportunity disposition command. It does not borrow a successor
Home or promise another firing. The existing reader applies durable successor
closure facts; closure still cannot cancel a running process or prevent its
fenced settlement. A later configuration change may supersede any observed retry
time; history preserves the observation as the last attempt's evidence.

## Preservation proof

The new CLI counterexample reached the installed cron executor, authored release
Flow and target lock in a disposable Home. Before repair it failed on the
generic continuation, `retry at the next configured firing after the active
execution finishes` (7.89s). The expanded final proof checks an exact due in
both the durable attempt and CLI log, JSON history, no candidate selection or
telemetry launch by the loser, no publication, unchanged tags and exact caller
HEAD, branch, index and edited/untracked bytes. The lock remains held throughout.

Accounting cases check same-day and later-day observations, cron overlap with an
unchanged active attempt, original-Home repair after closure, and the retained
Los Angeles timezone across its nonexistent spring calendar time. Closure is
seeded through the real accounting operation; it is not a concurrent live
reconfiguration demonstration. Git, CLI, file records and locks are real;
external services and historical dates are simulated. No qualifying configured
settlement is supplied by these fixtures.

The two accounting cases passed in 0.07s. The initial repaired CLI proof passed
in 10.56s; its final run also checks the physical verifier invocation count and
caller preservation after the history read. Final validation is recorded below.

## Review and remaining work

The simulated review kept the continuation as observed evidence rather than
adding a durable retry owner. An exact next due satisfies the overlap stop
without fabricating an active-attempt identity from the target lock. Repeated
reads neither settle work nor transfer execution authority. Existing settlement
fences still reject a late attempt.

Automatic continuation across closed segments, actual telemetry continuity and
dated repair ownership, remaining interruption proof, required UI/public checks
and two adjacent configured automatic settlements remain open. The observed
scorecard schema blocker and unaccepted Intelligence handoff remain unchanged.
Other waits are outside this cut: telemetry's existing retry messages still use
the entry snapshot's next due; this does not establish their accuracy after a
wait crosses that boundary. No production installation, cron sync/trigger,
publication, PM write, PR publication or Task completion occurred.

## Validation

- `cargo test -p loopflow --lib overlap_continuation_ -- --nocapture`:
  two passed, 0.07s execution (2m compilation).
- `cargo test -p loopflow --test scheduled_release_tests release_overlap_records_exact_next_firing_without_mutation -- --nocapture`:
  reproduced the missing exact continuation before repair; final expanded proof
  passed in 15.06s (0.83s compilation).
- `cargo fmt --check`, `git diff --check` and
  `cargo clippy --all-targets -- -D warnings`: passed; final Clippy took 0.48s.

Test commands cleared `LF_CONTROL_HOME`, `LF_CONTROL_DB_PATH`, `LF_HOME` and
`LF_DB_PATH`; the shared fixture also isolates ambient execution context. No
affected-suite gate or full CI run was performed. The unchanged accounting
cases were not rerun after adding only CLI assertions. PR and Task remain open.
