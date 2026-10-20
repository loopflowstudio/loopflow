# Reader floor: additive migrations stop locking out older lf

Jack approved this on 2026-10-19. It supersedes the earlier stance of "no
old-version support yet": the floor only relaxes a refusal, and nothing in it
reads two shapes of the same data.

## Problem

About 70% of releases in the last month (41 tags, 29 migration batches) shipped a
migration. Any migration makes every older lf refuse the store. Two checks are
exact matches:

- `pending_migrations` (`rust/loopflow/src/store/migrations.rs` ~1221) refuses
  any ledger id this binary doesn't know.
- `validate_schema` (~1292) compares the whole product schema for equality.

Of the 86 drafts in 0.12–0.14:

| Kind | Share |
|---|---|
| Additive | 33% |
| Backfill | 19% |
| Destructive | 48% |

Breaking stays common while the data model is being simplified, which is fine.
What has to change is that breaking becomes **visible and chosen**, and additive
releases stop costing a lockout.

## Outcome (the demo)

1. Build lf at the current release, then add an additive draft (a nullable
   column plus a new table) and cut it, in a test fixture rather than a real
   release.
2. The store migrated by the newer build still opens with the older binary.
   Its reads and writes work.
3. A destructive draft (drop or rename) raises the floor. The older binary
   refuses with one message naming the release that raised it, e.g.
   `this store needs lf 0.15.0 or newer; install it with lf install`.
4. The release-cut output and release notes carry
   `lf < 0.15.0 can no longer open the store` exactly when the floor rises, and
   name the drafts that raised it. `lf install promote` prints the same line
   before advancing.

## Design

**Ledger.**
- `schema_migrations` gains `reader_floor TEXT`: the oldest lf version that
  can open the store once this migration is applied.
- The canonicalizer computes it at the release cut, so it lives in the
  generated `Migration` entry and is written to the ledger when the migration
  is applied.
- This column is itself additive.
- Ledger rows written before it existed have a floor equal to their own
  version (unknown counts as breaking).

**Additivity is proven, never declared.**
- At the release cut, and in `check_migrations.py`, apply each draft on top of
  the released frontier and diff the product schemas.
- Additive means:
  - every old object survives byte-identical;
  - new objects are tables, indexes, or `ADD COLUMN` that is nullable or has a
    default;
  - there are no new triggers on existing tables and no `UPDATE`/`DELETE`.
- Anything else is breaking. A draft may still declare
  `-- breaks: <reason>` to force breaking for semantic changes, such as a new
  enum value in a TEXT status column or a new JSON field an older strict reader
  rejects. There is no way to declare a draft additive.
- A batch's floor is the max over its drafts: the release version if any
  draft breaks, otherwise the previous floor.

**Opening as an older binary** (`validate_sqlite_schema`, `pending_migrations`,
and the ordinary shared open in `store/sqlite.rs` ~575):
- Known ledger ids must still be an exact prefix, as now.
- An unknown tail is allowed when every unknown row's `reader_floor` is at or
  below this binary's `CARGO_PKG_VERSION`.
- The schema check becomes: this binary's expected schema ⊆ actual. Every
  expected object is present and identical, except tables, whose columns may be
  a superset.
- Otherwise it refuses with the one message above.
- This applies to ordinary opens. The promotion boundary still only advances,
  and an unknown tail there is still an error.

**Prerequisites, in this order (mechanical, ship them first in the same PR):**
1. No positional inserts. 35 `INSERT INTO <t> VALUES` in non-test
   `rust/loopflow/src` get explicit column lists. Etude bans this with a static
   test; add the same check to `check_migrations.py` or a Rust test.
2. No `SELECT *` in non-test code (8 sites). Name the columns.
3. Audit the 22 `#[serde(deny_unknown_fields)]` sites. Any type persisted into
   a JSON column drops it, so an older reader tolerates newer fields. Config
   files and CLI input may keep it.

**What CI can't prove, and new code must accept:** an older client may write
rows during the window, so they won't have the new columns. New code must read
`NULL` there sensibly. State this in `store/MIGRATIONS.md`.

**B: the breaking line.**
- `canonicalize_migrations.py --release-cut` writes the floor into the
  generated entry. When the floor rises, it prints and records
  `lf < X can no longer open the store (raised by: <drafts>)` so
  `lf repo release run` puts it in the release notes and the PR title.
- `lf install promote` prints it before advancing.
- This is visibility, not a gate: no flag is required.

## Concept review (2026-10-19)

**Accepted by Jack:** rollback works within the floor. After an additive
release, the retained older lf may be selected again. This replaces "an unknown
tail there is still an error" for candidates *behind* the store; promotion
still never applies or accepts unknown SQL.

- Experience: promote 0.15.1 (additive), find a bug, roll back to 0.15.0 →
  `store frontier 0.15.1.001_release; candidate is behind / reader floor 0.15.0
  admits it / VERDICT: promote (no migration to apply)`. A breaking release
  still refuses rollback with the store-raised message.
- Model: `Compatibility::Behind { applied_frontier, reader_floor }` (proposed
  shape). `classify_compatibility` uses `admitted_newer_tail` instead of
  `validate_known_history`; `decide` maps `Behind` to `Verdict::Promote`, so
  `validate_rollback_verdict` accepts it unchanged. A forward promotion over an
  admitted tail is a no-op select, which is the same thing.
- Why now: the preflight runs inside the older binary, so whatever this PR
  ships becomes every future rollback target's behavior.
- Next proof: install test seeding a store one additive release ahead of this
  binary → preview is `Behind` and the verdict `Promote`; with a breaking tail →
  `Incompatible` naming the release.

Implemented 2026-10-19 (`Compatibility::Behind`, install test
`rollback_stays_open_within_the_reader_floor`). The refusal names the floor once —
`this store needs lf 0.15.0 or newer; install it with lf install` — since the
floor is the release that raised it.

## Constraints

- Follow `rust/loopflow/src/store/MIGRATIONS.md`. The `reader_floor` column is
  this Task's one draft, created with
  `uv run python scripts/new_migration.py reader_floor`.
- Released migration files are immutable. Floors for already-released batches
  come from the "unknown means breaking" default, never from editing them.
- Tests:
  - Seed a released schema from `rust/loopflow/tests/fixtures/migrations/`.
  - Older-binary behavior is tested by opening with a migration set that is a
    prefix of the current one, through the existing `open_with`-style seams.
    No new factory traits.
- Update `store/MIGRATIONS.md` and the release-notes skill
  (`rust/loopflow/src/builtins/ops/skill/release-notes.md`) where they describe
  compatibility.

## Implementation notes

Delete — do not maintain: exact-only ordinary schema validation and unknown-tail
refusal on the ordinary open path. Preserve strict promotion
(`validate_known_history`), checksum checks, known-prefix ordering and migration
transaction boundaries.

- Prerequisites: no positional inserts existed in production Rust (the 35 hits
  were test fixtures); four production wildcard projections now name columns;
  `check_migrations.py` rejects both outside `#[cfg(test)]` modules. Persisted
  JSON types (WorkEdit and its values, PlanningMutation/Snapshot, WorkflowEdge,
  ConcretePath, WaveChapterPlan) tolerate added fields; config, authored
  definitions, CLI input, frontmatter and release planning inputs stay strict.
- Runtime: `admitted_newer_tail` + `validate_schema_within` on ordinary opens;
  exact schema whenever the ledger holds no newer tail.
- Release cut: the classifier lives in `canonicalize_migrations.py` (one module,
  so fixtures copying the script keep working); a raised floor goes to
  `release/unreleased/BREAKING.md`, which `lf repo release run` reads into the
  notes context, finalized notes and the PR title.
- Promotion: `Compatibility::AheadPending.raised_reader_floor`, printed by the
  preflight before advancing. An installed lf older than this change ignores
  the field, so the first promotion that prints it is the one after this ships.

Remaining: gate, then review.
