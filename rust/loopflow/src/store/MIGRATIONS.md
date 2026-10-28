# Schema migrations

```bash
uv run python scripts/new_migration.py add_wave_colour   # this Task's migration
uv run python scripts/check_migrations.py                # what CI runs
```

Write the SQL in `migrations/<name>.sql` and you are done — there is no registry
to edit. The file is the migration's registration, and its name is its identity:
the `schema_migrations.version` an applied store records.

## Order comes from main

`build.rs` orders the catalog: the released batches (`<version>.<NNN>_release.sql`,
frozen history) in id order, then every slug migration in the order
`origin/main`'s first-parent history added it, then this checkout's unmerged
slug last. The PR never picks a number and nothing renumbers at merge or
release; two Tasks adding migrations at once order themselves by which merged
first. Every build of one main commit computes the same order.

A source snapshot without Git history (a container build) passes the merged
order explicitly: `LOOPFLOW_MIGRATION_ORDER=<file>` lists one slug per line. A
shallow clone or a checkout with no `origin/main` treats every slug as unmerged.
Release builds refuse to compile with an unmerged migration.

## One migration per Task, edited in place

A Task keeps one migration until it merges. When the schema changes again,
rewrite that file to the final shape; `new_migration.py` prints the one the
branch already has instead of creating another. Test the upgrade from the last
merged schema to the finished migration, not the steps between.

Once a migration is on `origin/main`, stores may have applied it: it is
immutable. Fix it with a new forward migration, even when it merged an hour ago.

## Who applies what

| Store | Applies |
| --- | --- |
| A private `LF_HOME` | everything this build knows, unmerged included, on open |
| `~/.lf/loopflow.db`, ordinary open | nothing; refuses when this build knows a migration the store lacks |
| `~/.lf/loopflow.db`, install switch | pending merged migrations, after a backup, under the promotion lock |

To try schema work, run the branch's lf with a private `LF_HOME`. It starts
empty: the branch's migrations build a fresh store there, apart from the
Machine's data. Editing an unmerged migration that a private Home already
applied is a checksum mismatch; start a fresh `LF_HOME`. A branch build without
a new migration runs its own code on `~/.lf` directly. Install a main build with
`lf self install --main`; the install switch refuses a candidate whose pending
migrations `origin/main` does not carry.

The runner temporarily disables foreign-key actions around the transaction so a
SQLite table rebuild cannot cascade-delete child history. It runs
`PRAGMA foreign_key_check` before commit and restores enforcement afterward; a
migration that leaves a dangling reference rolls back as one unit.

That is the only place stored rows are scanned for dangling references during
ordinary use. Opening a store validates its migration ledger and schema and
relies on per-connection enforcement; `lf doctor` and installation
preflight run the full `PRAGMA foreign_key_check`, which reads the whole database.

Persisted JSON is schema too. Changing a required field, enum variant, or wire
shape in a DTO stored by the database requires a repair in the Task's migration and
a typed upgrade test seeded with the released shape. Before commit, the runner
deserializes every registered persisted JSON column into its current Rust type
and reports all incompatible rows together; any failure rolls back the complete
migration transaction.

Retired Machine landing supervision stays in `pr_landings.retired_supervisor_json`
with its original Machine, PID, heartbeat and generation. The migration advances
the claim generation and clears executable supervision without changing delivery
intent, failures or terminal history. Older executables cannot acquire another
Machine claim. Historical PIDs are evidence only; they must never be signaled locally.

Before advancing an existing on-disk database, the runner takes a SQLite backup
inside the same exclusive transaction and publishes it atomically beside the
database. The filename carries the previously applied migration and a fingerprint
of the complete ledger plus product schema, so a backup from another branch-local
history cannot be mistaken for the state being repaired. Once a migration
commits, the two newest backups with that fingerprinted name stay and older ones
are removed; a file named any other way is never touched.

## Older lf keeps opening an additively extended store

`build.rs` proves each slug migration additive by applying it over the schema
before it — every old object survives unchanged, new objects are tables or
non-unique indexes, new columns are nullable or defaulted, and no statement
writes an existing table. Force a semantic break, such as a new status value an
older reader rejects, with `-- breaks: <reason>`. Nothing declares a migration
additive.

A migration's reader floor is the newest breaking migration at or before it;
released batches and breaking migrations name themselves. Every ledger row
records it. A build opens a store holding migrations it does not know only when
it knows their floor, and then requires every object it expects to be present
unchanged; tables may have gained columns. Otherwise it refuses with
`this store needs an lf that knows drop_waves (it has drop_waves; latest known task_graphs); install it with \`lf self install --main\``.
Install previews name the breaking migration a candidate would apply. Promotion
never applies history it does not know; selecting an older build the floor
admits, as a rollback does, applies nothing and is allowed.

Rename, drop and reshape directly; the floor rises at that migration.

## Reader catalog

```bash
lf self install reader-catalog    # the floors this build knows, one per line
```

Every package (green main and release) ships this list as `reader-catalog.txt`
beside its archive. A build opens a store when its catalog holds the floor of
the store's newest migration; `store::migrations::can_open(store, catalog)`
answers that, so an updater can rule out a candidate before downloading it.

New code must accept what an older writer leaves behind. While older lf still
runs, it writes rows without the new columns, so read `NULL` there sensibly, and
avoid `INSERT OR REPLACE` on extended tables: an older writer's replace drops
the newer columns. Runtime SQL names its columns — `check_migrations.py` rejects
positional `INSERT ... VALUES` and `SELECT *` outside tests — and JSON persisted
in the store tolerates fields it does not know.

## What the check enforces

- Every file is a released batch or a `<snake_case_slug>.sql`; no subdirectories.
- Every migration on `origin/main` is present with the same bytes, and no new
  released batch appears.
- A slug never reuses a released migration's name.
- Runtime SQL names its columns: no positional `INSERT ... VALUES`, no `SELECT *`.

## What a database can be told

The build generates the schema reference from the ordered catalog using
the bundled SQLite library, then embeds its schema values in the binary. Validation
uses a per-process cache keyed by exact ordered migration SQL; the generated
reference seeds its full-catalog entry and historical prefixes construct theirs on
first use. Each validation
compares the actual database schema, including constraints, indexes, triggers and
foreign keys. The process reads that schema once per file and SQLite schema cookie
(`PRAGMA schema_version`, bumped by every committed DDL change from any process);
in-memory databases and reads inside a write transaction always read afresh.
Neither cache holds a validity result, an open SQLite connection or the ledger:
applied versions, checksums and reader floors are read on every open.

| State | Message |
| --- | --- |
| Behind the chain, private Home | applies the missing tail and continues |
| Behind the chain, `~/.lf/loopflow.db` | names the pending migration and `lf self install --main` (or a private `LF_HOME` for unmerged work) |
| Pre-namespace `001_initial` stamp | adopted as `0.10.001_initial` — same bytes, no data moved |
| Known leading migration names under branch-local ordinals | verifies the complete product schema, rewrites the ledger to canonical order, then continues |
| Newer migrations whose floor this build knows | opens; this build's schema must survive unchanged |
| Newer migrations whose floor this build does not know | names the migration the store needs |
| Skipped a migration, or drifted from the chain's schema | keep `loopflow.db`; open it with the lf that last migrated it, or `lf self install --main` |

Every refusal names a build that opens the store; none suggests deleting it.

## Why there is no separate "schema change without a migration" check

Schema exists only inside these files. The only way to change it without adding a
migration is to edit a shipped one — which the immutability check already rejects.
A second check would restate the first.
