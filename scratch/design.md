# LOO-563 — Verify the schema once per schema version, not once per open

Status: implemented (2026-10-21, commit 3e2e6d84b9, plus the `user_tables`
deletion in step 3); gate and ship remain. Jack Heart filed the Task while
landing LOO-497 and asked for the run in auto mode.

## Problem

Every `SqliteStore` open re-reads the whole product schema: one
`sqlite_master` scan, one `PRAGMA foreign_key_list` per table, and two
whole-table `user_tables` listings. A bare `lf` launch opens the store 17
times across three lf processes (15 in the launching process), so schema
verification is the largest share of launch work and it grows with every
table. LOO-497 needs four more tables and had to raise the startup budget
from 25,000 to 27,000 rows to land.

Beneficiary: anyone launching `lf` (every Session start, every nested `lf`
command an agent runs), and every future migration author who otherwise
pays a launch-budget tax per table.

## Demo

```
cargo test --test integration agent_startup -- --nocapture < /dev/null
[]: …, 1683 statements, 24638 rows            # before (main @ 53491fff86)
[]: …, 733 statements, 15740 rows             # after (implement, 2026-10-21)
```

The budget assertion in `rust/loopflow/tests/agent_startup_tests.rs` drops
from `< 25_000` to `< 18_000`, leaving LOO-497 room to land its tables
without raising it.

## Measured baseline (main @ 53491fff86, 2026-10-21)

Per-statement row attribution of the bare launch (temporary SQLite trace
probe, reverted):

| Source | Rows | Notes |
| --- | ---: | --- |
| SQLite internal schema load (rows with no statement SQL) | 11,025 | ≈441 rows per load, 25 loads over 17 connections |
| `product_schema` `sqlite_master` scan | 6,256 | 17 scans, one per open |
| `PRAGMA foreign_key_list` | 1,513 | 89 rows per scan |
| `user_tables` listing | 2,553 | ~2 per open, 69 rows each |
| `schema_migrations` ledger reads | 3,034 | versions, checksums, table_info |
| everything else (real work) | ≈257 | |
| **Total** | **24,638** | 1,683 statements |

Schema-object verification (`product_schema` + FK + `user_tables`) is
10,322 rows; all but one scan per process is repeated reading of an
unchanged schema.

## Chosen approach

Cache the *live schema read* per process, keyed on SQLite's own schema
cookie. Every comparison keeps running exactly as today against that value.

SQLite increments `PRAGMA schema_version` on every committed DDL change by
any connection or process; it is the mechanism SQLite itself uses to
invalidate prepared statements. Reading the schema of the same file at the
same cookie twice cannot return different objects, so the second read is
served from memory.

1. **One choke point: `product_schema`** (`store/migration_schema.rs`). It
   returns `Arc<[ProductSchemaObject]>` and consults
   `static LIVE_SCHEMAS: OnceLock<Mutex<HashMap<PathBuf, LiveSchema>>>`,
   `LiveSchema { cookie: SchemaCookie { dev, ino, schema_version }, schema:
   Arc<[ProductSchemaObject]> }`; `_committed_cookie` returns `None` for
   uncacheable connections. A hit needs the same path (`conn.path()`),
   the same `(dev, ino)` from `std::os::unix::fs::MetadataExt` and the same
   cookie. A miss reads `sqlite_master` and the FK pragmas as today and
   replaces the path's entry. The lock is held for lookup and insert only.
   Every caller — `validate_schema`, `validate_schema_within`,
   `_validate_development_schema`, `adopt_permuted_history`,
   `validate_divergent_schema`, `migration_history_fingerprint` and
   `expected_schema`'s own in-memory build — keeps its comparison unchanged.
2. **Neither consult nor record** when `conn.path()` is `None`/empty
   (in-memory and temp databases, including every `expected_schema` build)
   or `conn.transaction_state(None)` is `Write`. Migrations run under
   `BEGIN EXCLUSIVE`, which is a write transaction from its first statement,
   so no uncommitted DDL is ever served or stored. `transaction_state` is
   behind rusqlite's `modern_sqlite`, which `bundled` already enables; no
   Cargo change.
3. **Cheap table probes.** Replace `user_tables(conn)?.is_empty()` and
   `user_tables(…).any(== "schema_migrations")` on the open paths
   (`initialize_experimental_sqlite`, `_initialize_experiment_in`,
   `_validate_canonical_history_for_development`, `validate_sqlite_schema`,
   `latest_applied_version_sqlite`) with single-row `EXISTS` queries over
   `sqlite_master`. The budget counts rows *returned* (`TraceEvent::Row` in
   `performance.rs`), so each probe costs one row. `apply_set` and
   `requires_migration_sqlite` only ever asked "is the ledger there" and "how
   many tables", so they use the same probes (`has_ledger`,
   `user_table_count`) and `user_tables` is deleted outright; tests use
   `has_table(conn, name)`.
4. **Ledger stays per open.** Applied versions, checksums and reader floors
   are data, not schema; an `UPDATE schema_migrations SET checksum` leaves
   the cookie unchanged. They are still read and checked on every open
   (≈180 rows/open, growing one row per migration, not per table).
5. **Budget.** Lower `agent_startup_tests` to `rows < 18_000`.
6. **Docs.** Rewrite the `expected_schema` doc comment ("Every caller still
   reads … afresh") and `MIGRATIONS.md` "What a database can be told"
   (line ≈214, "The cache contains no database validity results"): the
   process caches the live schema per file and cookie, never a verdict,
   never a connection, never the ledger.

Decision (2026-10-21, kickoff revision): cache the read, not a verdict. The
earlier draft cached successful `product_schema == expected` verdicts keyed
additionally on `Arc::ptr_eq(expected)`. That covered only two of the six
comparison sites; `validate_schema_within` runs on every ordinary open of a
store a newer release migrated, and would have kept paying the full read.
Caching the read covers every site, needs no verdict semantics, and an
incompatible store still fails on every open with today's message (its
comparison is just in memory).

## Risk findings

- **Prior decision reversed, on purpose.** #1324 (2026-09-29) cached only
  the expected reference and stated that every validation reads the live
  schema afresh, backed by drift tests. This Task asks for verification
  once per process. The cookie key keeps the property those tests protect:
  any DDL drift bumps the cookie and forces a fresh read. The drift tests in
  `migrations.rs` (`warmed_schema_reference_still_detects_live_index_drift`,
  `expected_schema_reuse_does_not_accept_schema_ledger_or_data_drift`,
  `embedded_canonical_reference_matches_runtime_and_detects_drift`) use
  in-memory connections, which are never cached, so they keep exercising
  the full read unchanged.
- **Cross-process drift.** `lf install` promotion migrates the shared store
  from another process while a long-lived Session process holds the cache.
  Migration DDL bumps the cookie; the next open or
  `validate_current_schema` re-reads. A data-only ledger change is caught by
  the per-open ledger checks (step 4).
- **`writable_schema` tampering** edits `sqlite_master` without bumping the
  cookie. Nothing in the repo uses it (grep, 2026-10-21); a store edited that
  way is corrupt by construction and is still caught by a fresh process.
- **File replaced at the same path.** Path + `(dev, ino)` + cookie all have
  to match. Linux reuses inodes quickly, which is why the path is part of
  the key: test tempdirs have random paths. Experiment copies
  (`store/copy.rs`) clone or back up into new paths, created once. The
  online backup API bumps the destination's cookie past its previous value,
  so cookies on one file stay monotonic. Residual: delete-and-recreate at
  the same path, reused inode and an equal cookie with different DDL inside
  one process. Nothing in the repo does that; the cost would be SQL errors
  instead of the incompatibility message.
- **Path spelling is part of the key.** `conn.path()` returns the string the
  connection was opened with, so the same file opened through two spellings
  (relative, symlinked) holds two entries. Both are correct; only the hit
  rate drops. Every store open in the repo resolves the path first, so the
  launch sees one entry per file.
- **Per-table growth after this change.** Each new table or index now costs
  one row per SQLite internal schema load (25 per launch), one `sqlite_master`
  row and one FK pragma per process (3), instead of 17 scans, 17 FK pragmas
  and ~34 listing rows. LOO-497's four tables fit well inside the 2,260-row
  headroom under 18,000.
- **Ruled out: one shared connection per path.** Returning a clone of an
  already-open `SqliteStore` would also remove the 25 internal schema loads,
  but the store is `Arc<Mutex<Connection>>`: code that holds one store's lock
  while opening another would deadlock, and parallel readers would
  serialize. That is a separate, riskier change.
- **Ruled out: one-row ledger stamp** (`count(*)`, `max(rowid)`). It misses
  checksum `UPDATE`s, which the existing drift test asserts are caught.

## Scope

**Source of truth:** the SQLite file's schema (`sqlite_master`, FK pragmas)
and its `schema_migrations` ledger. `LIVE_SCHEMAS` is a per-process,
in-memory copy of the last committed read per file and cookie; it holds no
connections, no verdicts, and is never persisted.

**Affected surfaces:** `SqliteStore::open_with` (experimental and shared
paths), `SqliteStore::validate_current_schema`, `latest_version_sqlite`,
`lf doctor`'s full validation (it still reads foreign keys row by row).
No CLI output, wire DTO or Desktop surface changes. Error messages stay
the same.

**Absent and error states:** an incompatible store keeps failing on every
open with today's message. Metadata failure for the path, a failed cookie
read, or a failed schema read means no entry and today's error.

**Exclusions** (finding recorded, outside this Task):
- SQLite's internal schema load: ≈441 rows per load, 25 loads across 17
  connections (11,025 rows, 45% of the remaining launch). Single-connection
  processes load the schema twice. Reducing the connection count or the
  double load is a separate Task.
- Ledger reads (3,034 rows).
- Why the launching process opens the store 15 times.

## Delete — do not maintain

- `user_tables` and every caller (step 3); `has_user_tables` folds into `user_table_count`.
- The "contains no database validity results" promise in `expected_schema`'s
  doc comment and in `MIGRATIONS.md`.

Must survive: every existing drift/compat test in `migrations.rs` and
`sqlite.rs`, unchanged.

## Forbidden outcomes

- A second validation path ("fast" and "full") or a cached verdict. There
  is one schema reader; the cache only serves its result.
- Caching ledger reads, in-memory databases or write-transaction reads.
- Raising any budget, or loosening the drift tests to fit the cache.

## Internal slices

One indivisible change. This slice = all of steps 1–6.

Focused test (new, `migrations.rs` tests module, file-backed in a tempdir):
`cached_schema_is_reread_after_ddl_from_another_connection`: validate a
file store, then on a second connection `DROP INDEX` and validate again →
mismatch error; a third connection's rolled-back `CREATE INDEX` inside a
write transaction leaves the next validation passing.

## Done when

- A bare `lf` launch reads under 18,000 SQLite rows in
  `agent_startup_tests` with the budget lowered to `< 18_000` (baseline
  24,638).
- Index, column, trigger and ledger drift is still rejected on the next open
  after a store was verified in the same process.

## Checks (gate, headless)

- `cargo test --test integration agent_startup -- --nocapture < /dev/null`
  (inherited `LF_*` unset) → passes, prints rows < 18,000.
- `cargo test -p loopflow --lib store::` → passes.
- `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check`.

Check results:
- 2026-10-21 baseline: `cargo test --test integration agent_startup` → pass, 24,638 / 20,878 rows.
- 2026-10-21 implement: `agent_startup` (budget `< 18_000`) → pass, 15,740 / 13,825 rows; `cached_schema_is_reread_after_ddl_from_another_connection` → pass, fails with the cookie dropped from the key; `loopflow` lib `store::` → 342 pass (run from the built test binary; cargo rebuilt on each invocation under load); `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check` → clean.
- 2026-10-21 compress: `user_tables` deleted, cookie folded into `SchemaCookie`; `store::migrations` lib tests → 127 pass; `cargo fmt --check` → clean; clippy and affected suites left to gate.

Implementation notes: `build.rs` includes `migration_schema.rs`, so it now
serializes `&*schema`. The cookie is read before `sqlite_master`, so a DDL
commit between the two reads stores a newer schema under a cookie the file
never returns to.
