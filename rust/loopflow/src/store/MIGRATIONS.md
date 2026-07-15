# Schema migrations

```bash
uv run python scripts/new_migration.py add_wave_colour
uv run python scripts/check_migrations.py                # what CI and the release run
```

Write the SQL, paste the printed entry at the end of `MIGRATIONS` in
`migrations.rs`, and you are done. The store applies every migration a database
has not seen, in id order, exactly once.

The runner temporarily disables foreign-key actions around the transaction so a
SQLite table rebuild cannot cascade-delete child history. It runs
`PRAGMA foreign_key_check` before commit and restores enforcement afterward; a
migration that leaves a dangling reference rolls back as one unit.

Before advancing an existing on-disk database, the runner takes a SQLite backup
inside the same exclusive transaction and publishes it atomically beside the
database. The filename carries the previously applied migration, so at least the
previous schema generation remains available.

## The one rule

**A shipped migration is never edited, renamed, or deleted.** Databases in the
wild already ran it; changing the file changes their history, not their schema.
Repair a shipped schema with a new forward migration. `check_migrations.py`
compares every migration against the last release tag and fails the build if one
moved.

## What the check enforces

- The directory and the `MIGRATIONS` registry name the same migrations, with the
  same ids and names. A file nobody registered never runs; a registry entry whose
  id, name, and file disagree is a lie about what a database applied.
- The registry is in id order, and no id is namespaced ahead of the package version.
- Nothing that shipped in the last release tag has changed.

It runs in CI, and — because `lf release` cuts a tag from local state and never
reads a CI result — `lf release check` and `lf release run` run it themselves
before anything is cut. Same script, both paths.

## Identity

```
0.10.001_initial.sql
 │  │  │   └── name — part of the identity, so a rename is a break
 │  │  └────── ordinal, three digits, restarting in each namespace
 └──┴───────── namespace: package major.minor when authored
```

- Patch releases append into the current namespace; after a minor bump,
  subsequent migrations start a new one.
  `0.11.001` and `0.12.001` are distinct migrations.
- Order is the numeric tuple `(major, minor, ordinal)`, never a string sort —
  `0.9.001` precedes `0.10.001`, which lexical order would invert.
- The file stem *is* the `schema_migrations.version` string. `MigrationId` in
  `migrations.rs` is the only thing that formats or parses it.
- The active namespace comes from the workspace `Cargo.toml` version, so a
  migration authored ahead of the version is a release error, not a choice.

## What a database can be told

| State | Message |
| --- | --- |
| Behind the chain | applies the missing tail and continues |
| Pre-namespace `001_initial` stamp | adopted as `0.10.001_initial` — same bytes, no data moved |
| Carries an unknown id | reports the unknown and latest-known ids — a newer release or a divergent local build wrote it |
| Skipped a migration, or drifted from the chain's schema | *delete loopflow.db and rerun* |

## Why there is no separate "schema change without a migration" check

Schema exists only inside these files. The only way to change it without adding a
migration is to edit a shipped one — which the immutability check already rejects.
A second check would restate the first.
