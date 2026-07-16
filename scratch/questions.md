# Open questions — ENG-18

## `resolve_pm_errors_on_missing_id` flaked once, unexplained

Seen failing once in ~7 full `cargo test -p loopflow --lib` runs on this
branch, at `receipt.rs:684` — `put_pm_snapshot` returning an error inside the
test's own guarded temp store. Never reproduces in isolation (needs full-suite
parallelism); three consecutive full runs after the read-only-opener change
were clean, and two full runs on the base commit were clean.

Assumption made: **pre-existing and unrelated to this change.** The failing
assertion is a PM snapshot write into a per-test tempdir database that nothing
in the lineage path touches, and the final version of `parent_is_recorded`
opens the ledger read-only (no migration lock, no backup), so it cannot
contend with another test's writes.

Not chased further because it is outside the lineage scope this Task was
directed to hold. If it recurs, the suspect is shared process-global env
between tests that hold `test_env_lock()` and tests that don't — not the
journal.

## `lf doctor` on the real machine store is proven by snapshot, not in place

A development build cannot write the production database
(`guard_development_database`), so `0.11.026` cannot be applied to `~/.lf`
until a release carries it. Verified instead against a `.backup` snapshot of
the live production store — real data, the real seven failures, the real
binary. `lineage` goes `fail` → `ok`, 172,973 rows in and 173,089 out (the
delta is this verification run's own rows), $62.67 of recorded spend intact.

The check will flip green on the real store when the next release lf opens it.

## `capture` is still red — deliberately out of scope

`lf doctor` reports `capture: 1944 failure(s)` on the same store. The directive
scoped this Task to lineage and named capture a separate red signal. Untouched,
and still failing.
