# Make concurrent local ledger writes deterministic (ENG-7)

## Outcome

**PR #1030 owns this incident. This branch carries the proof, not a second fix.**

The root cause was found here, reported, and fixed independently by #1030
("store: skip no-op migration transactions", merged 2026-07-17). Re-measured
against main with **zero source changes**, the production-shaped fleet probe is
**zero-loss**. So this Task reduces to the regression guard that pins #1030's
behaviour, and nothing else.

## Problem

On 2026-07-16/17 a fleet of ~51 concurrent Loopflow/provider processes killed
live Session bodies with `task process failed: sqlite error: database is
locked`. W2-284, W2-285 and W2-287 parked failed on it; ENG-18 generation 3 and
W2-280 generation 1 died the same way; the supervising Developer Efficiency
Project Session failed twice with observations unconsumed. Failures are marked
resumable, so recovery was a human resuming each Session by hand — the avoidable
repair step Developer Efficiency's first KR says must fall to zero.

## The reproduction, and why the filed cause was wrong

The ticket blamed concurrent `lf pm` reads losing ledger receipts to write
contention. Measured, that is false.

| Scenario (fanout 51) | Result |
|---|---|
| Writers sharing one connection, 1020 inserts | **0 lost**, 4.9s |
| Same, 5100 inserts | **0 lost**, 7.3s |
| Writers with a **fresh connection per event**, 1020 inserts | **426 lost**, 61.8s |

Write contention on a live connection is a non-problem. The *open* was the
killer — and `journal::open_ledger()` opens a fresh connection **per run event**.

## The named cause

Every `SqliteStore::new` against an existing database ran
`apply_sqlite_with_backup` → `apply_sqlite_transaction`, which executed:

```
BEGIN EXCLUSIVE
  before_migration(conn)          -- no-ops when nothing is pending
  apply_set(conn, MIGRATIONS)     -- no-ops when nothing is pending
  validate_foreign_keys(conn)     -- PRAGMA foreign_key_check: FULL DATABASE SCAN
COMMIT
```

`requires_migration_sqlite` was only consulted *inside* `before_migration`, after
`BEGIN EXCLUSIVE` had already taken the exclusive write lock. So **every store
open took a global exclusive lock and held it for an O(database) foreign-key
scan, even with zero migrations pending.** At fanout 51 the fleet serialized
behind that lock and writers waited out `busy_timeout` and gave up.

**The pragmas were innocent.** WAL, `busy_timeout = 5000` and `foreign_keys = ON`
were all set and genuinely applied. The proof: across the failing run **zero
opens failed and every failure was an insert** — a busy handler that was
installed, waited its full 5s, and still found the lock held. A retry layer alone
would not have fixed this; it would have queued longer behind the same lock.

## What #1030 changed

It consults `requires_migration_sqlite` *before* opening the migration
transaction, so a current schema takes no exclusive lock:

```rust
let result = match requires_migration_sqlite(conn) {
    Ok(false) => Ok(()),
    Ok(true) => apply_sqlite_transaction(conn, |conn| ...),
    Err(error) => Err(error),
};
```

Same root cause, landed independently.

## Verification against main

Probe re-run on main, `git diff origin/main -- rust/loopflow/src/` **empty**:

| Metric | Before | On main (#1030) |
|---|---|---|
| Receipts lost, fanout 51, open-per-event, 1020 writes | 426 (41.8%) | **0** |
| Wall clock, same | 61.8s | **2.0s** |
| Receipts recorded exactly once | — | **1020 / 1020** |

Zero-loss. Per the Task's own condition, **no retry is warranted** and none is
added.

## What this branch ships

Two tests, and nothing else:

1. `every_receipt_at_fleet_fanout_is_recorded_exactly_once` — the fleet-scale
   proof at fanout 51 with a fresh connection per event. Asserts no write
   errored, `COUNT(*)` equals the requested total, and no `(run_id, seq)` is
   recorded twice.
2. `opening_a_current_database_takes_no_exclusive_lock` — the precise,
   timing-free guard for the named cause. Holds the write lock across a store
   open; asserts on the open's outcome, never on elapsed time, so it cannot
   flake under parallel load.

Writers open their own connections. SQLite locks per connection through file
locks and the WAL's shared memory, not per process, so separate connections in
one process contend exactly as separate processes do — which is what makes a
fleet-scale proof deterministic in one test binary.

### Sabotage proof

Per wave memory ("a test whose subject is 'the store says no' passes for free
against a store that does not exist"), both tests were sabotaged against main's
real code — forcing `requires_migration_sqlite` to always report a pending
migration, restoring pre-#1030 behaviour:

- `opening_a_current_database_takes_no_exclusive_lock` → **red** (deterministic).
- `every_receipt_at_fleet_fanout_is_recorded_exactly_once` → **red** (3 receipts
  lost).

Restored, both green. The tests pin #1030's behaviour, not a fixture.

## Discarded (deliberately)

An earlier iteration of this branch built a second fix on top of the root cause.
All of it is dropped: a per-process connection cache, a bounded jittered retry
ladder, a typed `StoreError::Contended`, `synchronous = NORMAL`, a
deep-validation rename, and a duplicate `schema_is_current` predicate. With the
probe zero-loss on main, none of it is load-bearing, and the retry in particular
was worse than nothing:

- Its test **passed for free** — an 80ms lock hold is absorbed by SQLite's own
  `busy_timeout`, so the test pinned SQLite's config and never reached the
  ladder. Sabotaging the ladder left it green.
- Each rung costs a full `busy_timeout` (5s) before `SQLITE_BUSY` is even
  returned, so a ladder trades a lost receipt for a multi-second hang.

## Open finding — not fixed here

**#1030 introduced a regression in the open path.** Its gate treats an
*uninitialized* database as "nothing to migrate": `requires_migration_sqlite`
returns `Ok(false)` both when the schema is current and when there are no user
tables at all. Combined with `existing_database` meaning only "the file has
bytes", a database file with a header but no tables — a process killed
mid-migration, which is reachable in a 51-process fleet — now permanently fails
to initialize with `no such table: run_events`, where the old code's `apply_set`
created the schema.

Verified by bisection, not inferred: the repro below **passes on `1e064a47b^`
and fails on `1e064a47b`**.

```rust
#[test]
fn an_existing_but_empty_database_still_gets_its_schema() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("loopflow.db");
    let conn = rusqlite::Connection::open(&path).unwrap();
    conn.execute_batch("PRAGMA user_version = 1;").unwrap();  // header, no tables
    drop(conn);
    assert!(std::fs::metadata(&path).unwrap().len() > 0);

    loopflow::store::sqlite::SqliteStore::new(&path)
        .expect("an existing but unmigrated database must still get its schema");
}
```

A permanent wedge needing a manual `rm loopflow.db` is an avoidable
human-in-the-loop repair step — the same KR this Task serves. It is a separate
defect from the contention incident, on a different code path, so it is filed
rather than absorbed here.
