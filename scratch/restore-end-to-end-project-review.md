# Review: Safe Project and Task controller execution

## What was implemented

Project and Task launches now settle an attempt-scoped startup receipt before
the public command succeeds. A running receipt links the Work, provider Run,
trace, Exec identity, PID, and OS birth; parked and failed receipts preserve the
other legitimate startup outcomes.

All controller status, signaling, recovery, and promotion paths use one derived
authority: a Work-linked Exec receipt whose PID and birth still identify a live
Loopflow process. Promotion captures those exact owners in its switch receipt,
quiesces them before store advance, and settles only after a distinct target
attempt is running or the Work is durably parked.

## Key choices

- Provider sessions, Runs, traces, and tmux remain useful evidence but cannot
  authorize a signal or veto recovery after positive OS absence.
- Release promotion stops and restarts managed controllers. Adjacent release
  generations never coexist for the same Work.
- `controller_handoffs: None` means capture has not happened; `Some([])` is
  durable evidence that capture completed with no live controllers.
- The promotion lock serializes launch against switching. A separate switch
  capability must name the active receipt before receipt-scoped recovery can
  bypass the shared launch lock.

## How it fits together

Public `project` and `task` start/resume commands call the shared Work launcher,
which waits for the child-owned startup receipt. The authority query joins that
receipt with immutable Exec evidence and a fresh OS process inspection. Release
promotion uses the same query to drive monotonic `captured -> quiesced ->
restarted` or `parked` handoffs in `SwitchReceipt`.

## Risks and bottlenecks

- Process inspection and birth validation are deliberately fail-closed. Missing
  or contradictory receipts can prevent recovery until the evidence is repaired.
- Startup waits up to ten seconds for the child receipt; immediate failures are
  durable and actionable, but a wedged child consumes that timeout.
- The local changed-aware gate could not start affected suites on 2026-09-02:
  the repository resource preflight found three active worktree build roots over
  the 12 GiB per-worktree budget (12.5 GiB, 12.5 GiB, and 18.1 GiB). Prescribed
  recovery retained them because their owners were active. This is an
  environmental validation gap, not a product-test failure; hosted CI must
  provide the final Rust and website suite results.

## What's not included

- Promotion does not claim arbitrary shells, provider processes, or other
  processes without exact controller ownership.
- This PR establishes the behavior and probes needed by the Project KRs; it
  does not itself supply the KRs' 30-day operating window or three live
  promotions.
- Hosted UI execution and E2E smoke remain their separately named gates.

## Validation

The production-shaped public Project/Task proof passed at the exact published
tree. It exercised real `lf` commands, SQLite stores, tmux controllers,
startup/Exec receipts, process ownership disagreement, stop/resume, phase
advancement, promotion, and interrupted-switch recovery.

- `cargo test -p loopflow --test controller_startup_tests public_project_and_task_controllers_prove_startup_and_resume -- --test-threads=1` — passed
- `cargo test -p loopflow machine_install::tests::controller_handoff --lib` — passed
- `cargo clippy -p loopflow --all-targets -- -D warnings` — passed
- `cargo fmt --all -- --check` — passed
- `git diff --check` — passed
- `uv run python scripts/test.py --reuse-passing` — stopped at resource preflight; product suites did not run
