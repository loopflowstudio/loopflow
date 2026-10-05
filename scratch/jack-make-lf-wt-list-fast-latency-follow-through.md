# LOO-375 PR 2: startup cost against a real store

Jack Heart's October 5 steer supplied the first installed sample (8.09 s; 4.91 s
startup, 1.90 s receipts) and asked for the largest remaining cost.

## Decision (2026-10-05, draft)

Ordinary store opens validate the migration ledger and schema and no longer run
`PRAGMA foreign_key_check`. Migrations, `lf home doctor` and installation
preflight keep the full scan. Evidence and numbers:
`scripts/benchmarks/wt-list/README.md` (2026-10-05 section).

## Installation tests leave the host suite (2026-10-05, Jack Heart's steer)

`lf home install preflight`/`promote` resolve the OS account's Home through
`getpwuid`, so three tests copied the live 1.1 GB database under live writers
and stalled the gate (terminated after 30 minutes; `.lf/tmp/gate-rerun/rust.log`:
2,100 passed, those three plus one signalled, 136 not run). They are now ignored
installation proofs listed in `scripts/test_task_installation.py`, which CI's
`task-installation` job runs in a disposable account. Production Home authority
is unchanged.

## Delete — do not maintain

Nothing slated. The five opens per process are now about 30 ms together; sharing
one connection is not worth a new owner.

## Remaining

- Read `lf wt timing` after a release carrying this is installed; that is the
  only measurement against the live Home and its writers. The Task stays open
  until then.
- ≤1 s warm p95 online is unmet: one GitHub round trip takes 1.4–1.8 s. Reaching
  it means answering PR state from something other than the remote, which the
  Task's truthfulness requirement does not obviously allow. Unselected.
- The three moved proofs have not run in the container: Docker was not running
  on this host. CI's `task-installation` job owns that result. PR #1444 is open
  at `c69eee058` with no hosted CI result yet; the terminated host run left 136
  Rust tests unrun, which hosted CI also owns.
- Installation preflight's store backup restarts whenever a writer commits, so
  on a large busy Home it has no bound. Observed only through these tests;
  unselected product work.
- Text-mode tail (one 10.9 s sample at load 79) is unexplained; ten samples.

## Checks

`cargo test -p loopflow --lib store::` 215 passed; `--test global_commands --test exec_ownership_tests` 17 passed, 5 ignored (inherited `LF_*` cleared); `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `ruff check` clean. Container proofs: CI's `task-installation`. Affected suites: gate.
