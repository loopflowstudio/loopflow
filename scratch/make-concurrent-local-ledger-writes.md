# Make concurrent local ledger writes deterministic (ENG-7)

## Problem

On 2026-07-16/17 the shared SQLite store, under ~51 concurrent Loopflow/provider
processes, terminated live Session bodies with `task process failed: sqlite
error: database is locked`. W2-284, W2-285 and W2-287 are parked failed on it;
ENG-18 generation 3 and W2-280 generation 1 died the same way; the supervising
Developer Efficiency Project Session failed twice with observations unconsumed.
Failures are marked resumable, so recovery today is a human resuming each
Session by hand — exactly the avoidable human-in-the-loop repair step
Developer Efficiency's first KR says must fall to zero.

**The filed reproduction blamed the wrong thing.** It said concurrent `lf pm`
read commands lose ledger receipts because writes contend. Measured, that is
false, and the real cause is somewhere else entirely.

## The reproduction (done first, before any design)

`rust/loopflow/tests/store_contention_repro.rs`, fanout 51. Each writer opens
its own connection — separate connections contend through the same file locks
whether or not they share a process, so this stands in faithfully for the
observed 51-process fleet without spawning 51 processes.

| Scenario | Result |
|---|---|
| 51 writers sharing one connection, 20 events each (1020 inserts) | **0 lost**, 4.9s |
| same, 100 events each (5100 inserts) | **0 lost**, 7.3s |
| 51 writers, **fresh connection per event** (1020 inserts) | **426 lost**, 61.8s |

Write contention on a live connection is a non-problem. The killer is the
*open*. That matters because `journal::open_ledger()` (`journal/mod.rs:464`)
calls `SqliteStore::new` — a brand-new connection — **per run event**.

## The named cause

Every `SqliteStore::new` against an existing database on a migration-authority
build runs `apply_sqlite_with_backup` → `apply_sqlite_transaction`
(`store/migrations.rs:423`), which unconditionally executes:

```
BEGIN EXCLUSIVE
  before_migration(conn)          -- no-ops when nothing is pending
  apply_set(conn, MIGRATIONS)     -- no-ops when nothing is pending
  validate_foreign_keys(conn)     -- PRAGMA foreign_key_check: FULL DATABASE SCAN
COMMIT
```

`requires_migration_sqlite` is only consulted *inside* `before_migration`, after
`BEGIN EXCLUSIVE` has already taken the database's exclusive write lock. So
**every store open takes a global exclusive lock and holds it for an O(database)
foreign-key scan, even when zero migrations are pending.** At fanout 51 the
fleet serializes behind that lock, and other writers' busy handlers wait out the
full `busy_timeout` and give up: `SQLITE_BUSY` → lost receipts → dead bodies.

**The pragmas are not wrong.** `journal_mode = WAL`, `busy_timeout = 5000` and
`foreign_keys = ON` are set and genuinely applied (`store/sqlite.rs:294`). The
proof: across the failing run, **zero opens failed and every failure was an
insert** — a busy handler that was installed, waited its full 5s, and still
found the lock held. A single wrong pragma does not explain this, and a retry
layer alone would not have fixed it.

**Causality confirmed by probe.** Gating the exclusive section on a cheap
`requires_migration_sqlite` read took loss from **426 → 0** at the same fanout.

### Second finding: the open path is O(schema) even when it does nothing

With the probe in place, loss went to zero but the run got *slower* (148.6s /
1020 opens ≈ **145ms per open**). `validate_sqlite` (`migrations.rs:405`) calls
`validate_schema`, which **replays every migration into a fresh in-memory
database** to diff against, then `validate_foreign_keys` scans the whole file.
Both run on every open. This is why read-only commands are not prompt, and it is
the same defect wearing a different hat: work that belongs to a migration is
being paid on every open.

## The demo

Start 51 concurrent `lf` commands against one store, then run `lf runs`: every
one of the 51 appears exactly once, no `ledger insert failed` warning is emitted,
and no Session is left `failed` needing a manual resume. Today that same fanout
silently drops ~40% of receipts. Alongside it, `time lf runs` on a warm store
drops from ~145ms of open overhead to single-digit ms.

## Approach

Fix the cause first, then make contention non-fatal by construction. Four
changes, in dependency order:

**1. A no-op open does no exclusive work.** Consult `requires_migration_sqlite`
(a cheap read) *before* `BEGIN EXCLUSIVE`, not inside it. When nothing is
pending, an open takes no exclusive lock, runs no foreign-key scan, and writes
nothing. This is semantically right, not merely faster: `validate_foreign_keys`
exists to validate *a migration's output* ("migration produced invalid foreign
keys") — with no migration, there is no output to validate.

The TOCTOU is benign: two processes that both observe "pending" still serialize
on the existing migration file lock and `BEGIN EXCLUSIVE`, and `apply_set` is
already idempotent. A process that observes "not pending" is correct to skip.

**2. A no-op open does no O(schema) work.** Full schema replay + FK scan move to
where they earn their keep: after a migration applies, and under `lf doctor`.
The steady-state open keeps `validate_applied_checksums` (reads
`schema_migrations`; cheap) which is what actually catches the drift class that
once cost 29 hours of writes. We keep the guard, we stop paying for it 51 times
a second.

**3. Stop the storm at its source.** `open_ledger()` caches its `SqliteStore`
per process, keyed by resolved ledger path. `SqliteStore` is already
`Arc<Mutex<Connection>>` + `Clone`, so this is a cache, not a redesign. Keying
by path (not a bare `OnceLock`) keeps `TestLedgerGuard`'s per-test homes
isolated. Per the first table, a shared connection at fanout 51 loses nothing —
so this alone removes the observed failure mode; changes 1 and 2 make the
remaining opens honest.

**4. Contention becomes bounded and explicit, never fatal.** A shared retry
helper wraps write operations: on `SQLITE_BUSY` / `SQLITE_LOCKED` /
`SQLITE_BUSY_SNAPSHOT`, retry with jittered exponential backoff to a bound; on
exhaustion return a typed `StoreError::Contended { operation, attempts, waited }`
rather than a bare `Sqlite(...)` string. Jitter matters: SQLite's own busy
handler is an unfair poll, and 51 unjittered retriers rethunder.

**Retry wraps the transaction, never the statement.** Retrying one failed
statement inside a multi-statement transaction would replay a partial write and
break exactly-once. The retry boundary is the whole closure, and only
transactions that are safe to replay from the start are wrapped.

**Where the body-death fix lives.** Done-when requires a Session body never dies
from contention alone, but the directive scopes me out of the Task lifecycle and
supervision paths. These reconcile: the store absorbs contention so it never
reaches the body as an error. No lifecycle code changes.

## De-risking

| Question | Finding | Impact on design |
|---|---|---|
| Are WAL / `busy_timeout` actually what we think? | Yes — set at `sqlite.rs:294` and verifiably applied: zero open failures, every failure an insert whose handler waited the full 5s. | Kills the "one wrong pragma" theory. The lock is genuinely held >5s; no retry layer would have saved it. |
| Is the filed cause (concurrent `lf pm` reads losing receipts to write contention) real? | No. 51 writers / 5100 inserts on live connections lose **0**. | Redirected the whole fix from the write path to the open path. |
| What actually breaks, then? | `BEGIN EXCLUSIVE` + whole-DB `foreign_key_check` on every open, migration pending or not. | Primary fix; everything else is secondary. |
| Is that really causal, or just correlated? | Probe (gate the exclusive section on `requires_migration_sqlite`): **426 → 0** lost at identical fanout. | Confirms the cause before a line of the fix is designed. |
| Does a bounded retry alone fix it? | No — and this is the trap. The lock is held for an O(db) scan by 51 serialized openers; retrying just queues longer behind the same lock. | Retry is defence-in-depth, not the fix. Ordering the work "cause first" is deliberate. |
| Can retry break exactly-once? | Yes, if it retried statements inside a transaction. A bare `INSERT` that returns BUSY never committed, so replay is safe; a partially-applied txn is not. | Retry boundary is the transaction closure, not the statement. |
| Why is a threads-not-processes test faithful? | SQLite locking is per-connection via file locks / WAL shm, not per-process. Separate connections in one process contend identically. | Deterministic fanout-51 test with no process fleet. |
| Would the test pass for free? | This is the live risk per wave memory ("a test whose subject is 'the store says no' passes for free against a store that does not exist"). The shared-connection variant *does* pass trivially — it was the open-per-event variant that reproduced. | Test must open per event, and must be proven by sabotage (below). |

## Alternatives considered

| Approach | Tradeoff | Why not |
|---|---|---|
| Just raise `busy_timeout` (5s → 30s) | One line | Treats the symptom. Turns a lost receipt into a 30s stall on every command while 51 openers serialize behind an O(db) scan. "Reads stay prompt" fails. |
| Only add a bounded retry layer, keep the open path | Matches the ticket's literal wording | The directive explicitly warns against this. Retrying into a held exclusive lock queues longer; it would have shipped a fix that does not fix. |
| Serialize all writes through one process/daemon | Removes contention by construction | A resident in the write path contradicts the wave's settled "db is the bus, as it is the registry" — publish must work with zero loopflow processes running. Large blast radius for a bug whose cause is a misplaced `BEGIN EXCLUSIVE`. |
| Global cross-process write mutex (file lock) | Simple, fair | Reintroduces exactly the serialization we are deleting, and adds stale-lock reaping and a `--force` everyone learns to reach for — the same argument that killed the worktree lease. |
| Drop schema validation from open entirely | Fastest open | Deletes a real guard; schema drift once cost 29 hours of writes. Keep the cheap checksum check; move only the expensive replay/scan. |

## Key decisions

- **Cause before cure.** The exclusive-lock-on-every-open fix lands first and
  carries the win on its own. The retry layer is defence-in-depth. Shipping the
  retry alone would have satisfied the ticket's wording and left the incident
  live.
- **`validate_foreign_keys` is migration-output validation, not an open-time
  invariant.** Running it per open was always a category error; it only looked
  like a performance bug.
- **Keep the drift guard, move it.** Checksum validation stays on the open path;
  full replay + FK scan move to migration-apply and `lf doctor`.
- **`synchronous = NORMAL` under WAL.** Standard SQLite guidance for WAL: still
  crash-safe for process death, loses only the last commits on host power loss.
  For a local dev ledger that is the right trade, and it cuts an fsync from every
  commit. Stated explicitly because it *is* a durability trade.
- **Typed `StoreError::Contended`.** Exhaustion must be nameable, not a
  stringly-matched `"database is locked"`. Callers and tests match the variant.
- **Retry at the transaction boundary only**, so exactly-once survives replay.

## Scope

**In scope**
- `store/migrations.rs`: gate the exclusive section on a pending-migration read;
  relocate full validation off the steady-state open.
- `store/sqlite.rs`: pragmas (`synchronous = NORMAL`), bounded jittered retry
  helper, `StoreError::Contended`.
- `journal/mod.rs`: per-process ledger store cache keyed by path.
- The deterministic fanout-51 concurrency test + its sabotage proof.

**Out of scope**
- Repairing W2-284, W2-285, W2-287 — parked deliberately; they resume once this
  lands.
- Task lifecycle, PR ledger, supervision paths. Contention is absorbed at the
  store; no lifecycle code changes.
- Any resident/daemon in the write path.

## Done when

1. `cargo test -p loopflow --test store_contention` is green: at fanout **51**
   with a fresh connection per event, **every requested receipt is recorded
   exactly once** — asserted as both "no write returned an error" and
   `COUNT(*) == expected` with no duplicate `(run_id, seq)`.
2. **Sabotage proof** (wave memory: a test that never contends passes for free —
   and this test's own shared-connection variant demonstrably does). Reverting
   the `requires_migration_sqlite` gate must take the test **red**, and the
   commit must record the observed failure count. A test that stays green under
   sabotage is pinning a fixture, not the behaviour.
3. Exhaustion is explicit: a test drives the retry bound to exhaustion and
   asserts `StoreError::Contended`, not a bare sqlite string.
4. Reads stay prompt: warm-store open overhead back to single-digit ms from
   ~145ms.
5. No Session body dies from contention: at fanout 51, zero store errors reach a
   caller.

## Measure

Baseline captured on this branch, 2026-07-16, `store_contention_repro.rs`:

| Metric | Before | Target |
|---|---|---|
| Receipts lost, fanout 51, open-per-event, 1020 writes | **426 (41.8%)** | **0** |
| Wall clock, same | 61.8s | comfortably under, no writer starving |
| Per-open overhead, warm store, nothing pending | ~145ms | single-digit ms |
| Opens taking `BEGIN EXCLUSIVE` with 0 migrations pending | 100% | 0% |

Re-run the same harness after the fix; the numbers are the proof.
