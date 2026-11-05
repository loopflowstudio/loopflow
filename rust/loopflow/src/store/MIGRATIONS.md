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

`lf install` prints the store advance and every live lf process by binary before
it changes anything, then proceeds; its result prints last, after skill sync, so a
`tail` of the output shows it. Processes on the previous lf keep their binary and
read the store the install leaves behind; a Session on one comes back through
`lf resume` once its client is stopped and that lf has exited. The install
records itself as a Process once the store has settled.

To try schema work, run the branch's lf with a private `LF_HOME`. It starts
empty: the branch's migrations build a fresh store there, apart from the
Machine's data. Editing an unmerged migration that a private Home already
applied is a checksum mismatch; start a fresh `LF_HOME`. A branch build without
a new migration runs its own code on `~/.lf` directly. Install a main build with
`lf install --dev`; the install switch refuses a candidate whose pending
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
database as `loopflow.db.backup-<previous migration>-<16 hex>`, never under
`~/.lf/backups`. The name carries the previously applied migration and a
fingerprint of the complete ledger plus product schema, so a backup from another
branch-local history cannot be mistaken for the state being repaired. Once a
migration commits, only the clone it made stays. When the install settles it goes
too, unless a process still runs an older build (`rollover::inventory()`, the list
`lf doctor` and `lf install` print): that process reads the live store, and the
clone is the restore point for the generation its build was written against, so
the clone stays while such a process can still show the migration lost something,
until the next install's commit. A file named any other way is never touched.

## Older lf keeps opening a newer store

Every build opens any store whose history extends its own. A store a newer lf
migrated has more ledger rows than this build knows; this build checks the rows
it knows and runs against the superset. Nothing is compared past its own
frontier, and nothing a migration does decides whether an older lf opens.

So a migration must leave older lf working: destructive change is staged.
Jack Heart's rule (2026-10-27): expand, wait for older lf to exit, export, drop.
A new migration is additive against `origin/main`'s schema, or it is a contract:

```sql
-- expand: wave_hue_colour                 # the migration that merged the new shape
-- exports: ~/.lf/exports/waves-hue.json   # where the preflight wrote what this drops
ALTER TABLE waves DROP COLUMN hue;
```

An expand step with no SQL to add, because the new shape is files or code as
`settings_authority`'s was, is a migration whose body is only a comment saying
what the store stopped being the authority for. It merges, starts the clock, and
the ledger records it.

`check_migrations.py` applies the branch's migrations over `origin/main`'s and
compares shapes. Additive: every table keeps its columns (a rebuild that changes a
CHECK and copies every column counts), every view keeps its columns (its definition
may change), new columns are nullable or defaulted, and no unique index an older
writer can violate joins an existing table. An older writer's row carries every new
column's default, so a unique index over only new columns whose `WHERE` is false for
those defaults never sees it. Trigger and index changes and data writes are additive
too. A type change, a column made `NOT NULL` or a required column without a default fails:
stage it as an expand now and a contract later. A contract may drop tables, views
and their columns that its expand step superseded, once that step has been on
`origin/main` for the staging window (`STAGING_DAYS` in `check_migrations.py`), and
it names where its preflight exported the data before the SQL ran. The settings
cutover is the template: `config::migrate::preflight` writes `settings-cutover.json`
under the migration fence, so an interrupted upgrade still has every document it
replaced.
The check measures; nothing at runtime refuses to install or migrate.

Rename and reshape are expand then contract. A process older than the window
meets a dropped column as an error from the one statement that reads it; the
rest of the store still answers.

New code must accept what an older writer leaves behind. While older lf still
runs, it writes rows without the new columns, so read `NULL` there sensibly, and
avoid `INSERT OR REPLACE` on extended tables: an older writer's replace drops
the newer columns. Runtime SQL names its columns — `check_migrations.py` rejects
positional `INSERT ... VALUES` and `SELECT *` outside tests — and JSON persisted
in the store tolerates fields it does not know.

`schema_migrations.reader_floor` is shipped history this build never reads. Each
new row repeats the ledger's newest floor, because builds released before its
deletion check every row past their own head and refuse one with none; Task
f5981e removes the write 30 days after that release.

## What the check enforces

- Every file is a released batch or a `<snake_case_slug>.sql`; no subdirectories.
- Every migration on `origin/main` is present with the same bytes, and no new
  released batch appears.
- A slug never reuses a released migration's name.
- Runtime SQL names its columns: no positional `INSERT ... VALUES`, no `SELECT *`.
- An unmerged migration is additive against `origin/main`'s schema, or a contract
  whose `-- expand:` step merged `STAGING_DAYS` ago and whose `-- exports:` names
  where the dropped data went.

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
applied versions and checksums are read on every open.

| State | Message |
| --- | --- |
| Behind the chain, private Home | applies the missing tail and continues |
| Behind the chain, `~/.lf/loopflow.db` | names the pending migration and `lf install`, which finishes installing this build (or a private `LF_HOME` for unmerged work) |
| Pre-namespace `001_initial` stamp | adopted as `0.10.001_initial` — same bytes, no data moved |
| Known leading migration names under branch-local ordinals | verifies the complete product schema, rewrites the ledger to canonical order, then continues |
| Newer migrations after this build's history | opens; applies nothing and compares no schema |
| Skipped a migration, or drifted from the chain's schema | keep `loopflow.db`; open it with the lf that last migrated it (named from the ledger when that build recorded itself), or run `lf install` |

Every refusal names a build that opens the store; none suggests deleting it.
Only a build newer than the store is refused for being out of step, and only a
fresh process meets that.

## Why there is no separate "schema change without a migration" check

Schema exists only inside these files. The only way to change it without adding a
migration is to edit a shipped one — which the immutability check already rejects.
A second check would restate the first.
