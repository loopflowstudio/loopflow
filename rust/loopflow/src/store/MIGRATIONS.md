# Schema migrations

```bash
uv run python scripts/new_migration.py add_wave_colour   # this Task's draft (no ordinal)
uv run python scripts/check_migrations.py                # what CI and the release run
```

Write the SQL in `migrations/drafts/<name>.sql`, and you are done — there is
nothing to paste into `migration_catalog.rs`. The file *is* the draft's
registration and its name is the draft's identity: canonicalization discovers it by
scanning the directory, and Rust never sees it until the release cut appends the
canonical `Migration` entry it generates.

## One draft per Task, edited in place

A Task keeps one draft until it lands. When the schema changes again, rewrite that
file to the final shape; run `new_migration.py` again and it prints the draft the
branch already has instead of creating another. Nothing has applied an unreleased
draft except disposable `LF_HOME` experiments, so there is no intermediate schema
to preserve, migrate from, or test. The same holds for another Task's unreleased
draft already on main: to change or undo it, edit it — do not add a draft that
alters or drops what it created.

A draft has no ordinal, so concurrent branches never contend, renumber, or share a
registry edit. Ordering that matters between Tasks — a draft that must run after
another draft or after an already-released migration — is declared with
`--depends-on` (a `-- depends_on:` header), not by a serial number.

Test the upgrade from the released frontier to the finished draft. A custom
`LF_HOME` is initialized once with the build's exact schema; after editing a
draft, start a fresh one.

## The release cut assigns canonical ids

The release PR is the single publication boundary that turns drafts into canonical
migrations. `lf repo release run` invokes the canonicalizer with `--release-cut` inside the
release worktree, **after the version bump and before the commit**, so the generated
files are part of the release PR and run under real Rust CI before the queue merges
and tags. It freezes the draft set, rejects missing or cyclic dependencies, and topologically orders it (edges
first, ties broken by name — never merge time, PR number, or wall clock). It then
concatenates the ordered bodies into a
`<major>.<minor>.<patch>.<ordinal>_release.sql` batch, starting at `001`. `-- draft: <name>` markers retain
the draft names for dependencies and incident review. The cut appends one
`Migration` entry and deletes all consumed drafts atomically; on any failure it
restores the tree byte-for-byte. The same drafts and version always produce the
same id and diff, so an aborted release regenerates identically. If migrations arrive after the first preparation commit, a corrected release cut
appends the next batch in the same unpublished version. Earlier batches remain
byte-for-byte immutable; a preparation commit does not consume a version. The manual script is a `--check` preview
only; creating canonical files requires `--release-cut`. Rust CI uses the separate
`--materialize-for-tests` authority in its disposable checkout. The release run is
the authority that publishes.

Only the merged release commit is canonical migration authority. Between releases,
ordinary merges add drafts, so main's canonical set does not move and a branch
that is merely behind main — adding only drafts — stays green.

Rust CI materializes the draft set in its disposable checkout before running the
test suite. This exercises the same deterministic schema and generated registry
the release cut would produce without publishing either one. The checkout is
discarded after the job; if the active version already has a canonical batch,
the materializer advances the disposable package to the next patch namespace.
Source builds and the shared store still see only canonical migrations from a
merged release.

The runner temporarily disables foreign-key actions around the transaction so a
SQLite table rebuild cannot cascade-delete child history. It runs
`PRAGMA foreign_key_check` before commit and restores enforcement afterward; a
migration that leaves a dangling reference rolls back as one unit.

That is the only place stored rows are scanned for dangling references during
ordinary use. Opening a store validates its migration ledger and schema and
relies on per-connection enforcement; `lf machine doctor` and installation
preflight run the full `PRAGMA foreign_key_check`, which reads the whole database.

Persisted JSON is schema too. Changing a required field, enum variant, or wire
shape in a DTO stored by the database requires a repair in the Task's draft and
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

## The one rule

**A published migration is never edited, renamed, or deleted.** `origin/main`
is the publication boundary because creator dogfooding runs ahead of patch tags.
Databases may already have run it; changing the file changes their history, not
their schema. Repair a published schema with a new forward migration.
`check_migrations.py` compares every migration against both `origin/main` and
the last release tag and fails the build if one moved.

## What the check enforces

- The directory and the `MIGRATIONS` registry name the same migrations, with the
  same ids and names. A file nobody registered never runs; a registry entry whose
  id, name, and file disagree is a lie about what a database applied.
- The registry is in id order. New canonical ids match the full package version,
  use increasing ordinals starting at `001`, and name each batch `release`. Known historical
  three-part ids remain valid; no new one can be introduced.
- Every canonical migration already on `origin/main` has the same ordinal, name, and
  bytes.
- Nothing that shipped in the last release tag has changed.
- Every draft under `drafts/` is well-formed: a snake_case `<name>.sql`, no
  collision with a released migration name, and a `depends_on` graph that resolves
  to other drafts or released migrations with no cycle. Drafts have no ordinal, so they
  are never compared against `origin/main`.

It runs in CI, and — because `lf repo release` cuts a tag from local state and never
reads a CI result — `lf repo release check` and `lf repo release run` run it themselves
before anything is cut. Same script, both paths.

## Identity

```text
0.12.2.001_release.sql
 │  │ │  │   └── canonical batch name
 │  │ │  └────── ordinal; corrected cuts append 002, 003, …
 └──┴─┴───────── package major.minor.patch that published the batch
```

- Historical `0.10.001_initial.sql` / `0.11.037_*.sql` ids remain immutable
  three-part ids. New canonical migrations always use the four-part format.
- Order is numeric: `(major, minor, patch, ordinal)`, with legacy ids before
  release-scoped ids in the same major/minor line. A binary skipped from `1.1.0`
  to `1.2.0` applies every missing patch and minor batch in that order.
- The file stem *is* the `schema_migrations.version` string. `MigrationId` in
  `migration_catalog.rs` is the only thing that formats or parses it.
- The ledger records the SQL checksum, parent-history fingerprint, build
  provenance, source checkout and revision, and package version. Old canonical
  rows receive checksums when the provenance migration first runs; their writer
  is intentionally left unknown rather than attributed to the upgrading build.
- The active namespace is the full workspace `Cargo.toml` version, so a batch
  authored for an earlier or later package release is an error, not a choice.

## What a database can be told

The build generates the canonical schema reference from the registered SQL using
the bundled SQLite engine, then embeds its schema values in the binary. Canonical
and development validation share the existing per-process cache keyed by exact
ordered migration SQL; the generated reference seeds its canonical entry. Drafts
and historical prefixes construct their reference on first use. Each validation still reads the
actual database schema, including constraints, indexes, triggers and foreign keys.
The cache contains no database validity results or open SQLite connections.

| State | Message |
| --- | --- |
| Behind the chain | applies the missing tail and continues |
| Unpublished build against `~/.lf/loopflow.db` | validates the applied prefix without running its pending migrations |
| Pre-namespace `001_initial` stamp | adopted as `0.10.001_initial` — same bytes, no data moved |
| Known leading migration names under branch-local ordinals | verifies the complete product schema, rewrites the ledger to canonical order, then continues |
| Carries an unknown id | reports the unknown and latest-known ids — a newer release or a divergent local build wrote it |
| Skipped a migration, or drifted from the chain's schema | *delete loopflow.db and rerun* |

## Why there is no separate "schema change without a migration" check

Schema exists only inside these files. The only way to change it without adding a
migration is to edit a shipped one — which the immutability check already rejects.
A second check would restate the first.
