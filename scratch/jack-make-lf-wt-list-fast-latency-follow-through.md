# LOO-375 PR 2: startup cost against a real store

Jack Heart's October 5 steer supplied the first installed sample (8.09 s; 4.91 s
startup, 1.90 s receipts) and asked for the largest remaining cost.

## Decision (2026-10-05, draft)

Ordinary store opens validate the migration ledger and schema and no longer run
`PRAGMA foreign_key_check`. Migrations, `lf home doctor` and installation
preflight keep the full scan. Evidence and numbers:
`scripts/benchmarks/wt-list/README.md` (2026-10-05 section).

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
- Text-mode tail (one 10.9 s sample at load 79) is unexplained; ten samples.

## Checks

`cargo test -p loopflow --lib store::` 215 passed; `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` clean. Affected suites belong to gate.
