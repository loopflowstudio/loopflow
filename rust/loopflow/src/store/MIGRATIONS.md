# Schema migrations

```bash
uv run python scripts/new_migration.py add_wave_colour   # author a draft (no ordinal)
uv run python scripts/check_migrations.py                # what CI and the release run
```

Write the SQL below the header, and you are done — there is nothing to paste into
`migrations.rs`. A **draft** carries a stable snake_case name, an immutable
authoring id (a 128-bit token, 32 hex chars), and **no ordinal**; it lives at
`migrations/drafts/<name>__<id>.sql`.
The draft's file *is* its registration: canonicalization discovers it by scanning
the directory, and Rust never sees it until the release cut appends the canonical
`Migration` entry it generates. Because a draft has no ordinal and two branches
authoring the same name mint different 128-bit ids (materially collision-resistant),
concurrent branches never contend,
renumber, or share a registry edit, and the allocator performs no `git fetch` or
rebase. Ordering that matters — a data migration that must run after another — is
declared with `--depends-on` (naming another draft or an already-released
migration), not by a serial number.

## The release cut assigns canonical ids

The release PR is the single publication boundary that turns drafts into canonical
migrations. `lf release run` invokes the canonicalizer with `--release-cut` inside the
release worktree, **after the version bump and before the commit**, so the generated
files are part of the release PR and run under real Rust CI before the queue merges
and tags. It freezes the draft set, rejects missing or cyclic dependencies (and two
drafts sharing a readable name in one cut), topologically orders it (edges first,
ties broken by name — never merge time, PR number, or wall clock), and assigns the
next contiguous ordinals in the namespace of the version being cut (a patch continues
the current `<major>.<minor>`; a minor bump starts a fresh sequence at ordinal 1). It
plans the whole tail in memory, then installs it atomically — writing
`<major>.<minor>.<ordinal>_<name>.sql`, appending the `Migration` entry, and deleting
the draft — and on any failure restores the tree byte-for-byte. Same drafts and
version always produce the same ids and diff, so an aborted release regenerates
identically. The manual script is a `--check` preview only; creating canonical files
requires `--release-cut`. Rust CI uses the separate `--materialize-for-tests`
authority in its disposable checkout. The release run is the authority that
publishes.

Only the merged release commit is canonical migration authority. Between releases,
ordinary merges add drafts, so main's canonical set does not move and a branch
that is merely behind main — adding only drafts — stays green.

Rust CI materializes the draft set in its disposable checkout before running the
test suite. This exercises the same deterministic schema and generated registry
the release cut would produce without publishing either one. The checkout is
discarded after the job; source builds and the shared store still see only
canonical migrations from a merged release.

The runner temporarily disables foreign-key actions around the transaction so a
SQLite table rebuild cannot cascade-delete child history. It runs
`PRAGMA foreign_key_check` before commit and restores enforcement afterward; a
migration that leaves a dangling reference rolls back as one unit.

Before advancing an existing on-disk database, the runner takes a SQLite backup
inside the same exclusive transaction and publishes it atomically beside the
database. The filename carries the previously applied migration and a fingerprint
of the complete ledger plus product schema, so a backup from another branch-local
history cannot be mistaken for the state being repaired.

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
- The registry is in id order. No id is namespaced ahead of the package version,
  and no new canonical migration is introduced behind the active package namespace.
- Every canonical migration already on `origin/main` has the same ordinal, name, and
  bytes.
- Nothing that shipped in the last release tag has changed.
- Every draft under `drafts/` is well-formed: a snake_case name matching its file, a
  `-- name:` header, no collision with a released migration name, and a `depends_on`
  graph that resolves to other drafts with no cycle. Drafts have no ordinal, so they
  are never compared against `origin/main`.

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
- The ledger records the SQL checksum, parent-history fingerprint, build
  provenance, source checkout and revision, and package version. Old canonical
  rows receive checksums when the provenance migration first runs; their writer
  is intentionally left unknown rather than attributed to the upgrading build.
- The active namespace comes from the workspace `Cargo.toml` version, so a
  migration authored ahead of the version is a release error, not a choice.

## What a database can be told

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
