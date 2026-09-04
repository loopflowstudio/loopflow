## Evaluate

Run:

```bash
cargo test -p loopflow --test controller_startup_tests public_project_and_task_controllers_prove_startup_and_resume -- --test-threads=1
cargo test -p loopflow machine_install::tests::controller_handoff --lib
```

The first proof drives public Project and Task commands through startup,
intentional stop/resume, phase advancement, process/provider disagreement, live
release promotion, and interrupted-switch recovery. It observes one captured
prior attempt, positive absence before store advance, and one distinct live
target attempt. The second proof rejects mutable or incomplete terminal handoff
evidence. Both pass.

## Why it matters

A detached tmux receipt previously let `lf task resume` report success even
when the controller died before creating a Run or entering its flow. Controllers
now have one exact operational authority and promotions preserve that ownership
across releases, so a false launch cannot strand Work or make an unrelated PID
signalable.

## What changed

- Added attempt-scoped running, parked, and failed startup receipts shared by
  Project and Task launch paths.
- Made a Work-linked, birth-validated OS Exec owner the sole authority for
  liveness and signaling; provider and Run state remain advisory.
- Persisted monotonic controller handoffs in the machine switch receipt and
  serialized ordinary launches against promotion.
- Quiesced exact prior owners before store advance, restarted captured Work
  through the selected target release, and made interrupted recovery converge
  through the same receipt.
- Documented the stop-and-restart release model and its fail-closed boundaries.

## Risks / Not included

Missing or contradictory ownership intentionally rejects signaling and
promotion rather than guessing. Arbitrary shells and provider processes are not
managed controller owners. The long-running Project KRs still require 30 days
of operation and three observed promotions.

The local changed-aware affected-suite gate was blocked before product tests by
three active worktree build roots exceeding the repository's per-worktree
resource budget; safe recovery retained them because their owners were active.
The exact-head focused behavior, handoff tests, formatting, and clippy pass;
hosted CI must provide the final affected Rust and website suite result.
