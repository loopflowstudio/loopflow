# Make migration histories safe under continuous dogfooding (W2-218)

## Problem, in one paragraph

`lf 0.11.1` rejects `~/.lf/loopflow.db` because its migration ledger is a
reordering of the canonical chain: ordinals 009–012 carry the same five
migration *names* as main but in a swapped order (live `009_profiles,
010_provider_account_lifecycle, 011_context_pressure,
012_context_input_normalization` vs. main `009_context_pressure,
010_context_input_normalization, 011_profiles, 012_provider_account_lifecycle`).
`pending_migrations` requires the applied versions to be an exact string prefix
of the registry, so a permuted-but-schema-identical ledger is fatal, and the
outage spreads to unrelated commands because every `SqliteStore::new` validates
the whole chain. The existing `adopt_divergent_history` converges exactly one
older hardcoded lineage and cannot see this one.

The root cause (`Why 8`): migration identity is a dense per-worktree ordinal
(`scripts/new_migration.py` takes `max(ordinal)+1` over the local tree), so two
branches forked from one main assign different schemas the same irreversible id,
and recognition of that id is a prerequisite for opening the store at all.

## User-visible outcome

- **Recovery:** a creator whose live store carries the permuted ledger runs any
  ordinary `lf` command (the open path auto-repairs) or `lf doctor`, and the
  store opens, upgrades through `0.11.014_task_lifecycle`, and passes SQLite
  integrity — with a fresh, history-fingerprinted backup written first. No
  hand-editing of `schema_migrations`, no `delete loopflow.db`.
- **Tolerance:** any *future* ledger that is a reordering of a known migration
  name-set (the shape both observed divergences take) self-heals on next open
  instead of bricking the CLI.
- **Prevention:** two branches can no longer assign the same ordinal to
  different migrations without CI catching it, every applied migration records
  who wrote it (build provenance, source identity, package version, content
  checksum), and `new_migration.py` allocates against the integration branch
  rather than the local worktree.

## Source of truth

`schema_migrations(version TEXT PK, applied_at INTEGER)` in the SQLite store is
the authoritative ledger; `MIGRATIONS: &[Migration]` in
`rust/loopflow/src/store/migrations.rs` is the canonical chain and its total
order. The filesystem `src/store/migrations/*.sql` files are the migration
bodies; the ledger records which have run and when. Everything else (doctor,
`latest_version_sqlite`, backups) is derived from these.

## Design

### PR 1 — Recover the live lineage; generalize convergence (forward-only, tested)

Lands first; independently unblocks the creator's store.

1. **Strengthen `product_schema` into a complete fingerprint.** Today it reads
   only table names + column names (`PRAGMA table_info` col 1). Before any
   generalized convergence can be trusted (Why-7 open question), compare the
   normalized `sqlite_master.sql` for every table, index, and trigger plus
   `PRAGMA foreign_key_list` — i.e. types, `NOT NULL`, defaults, PK, uniqueness,
   indexes, triggers, and FKs. This is the equivalence test the whole repair
   leans on; weak equivalence is how a subtly-wrong DB would be adopted.

2. **Replace the single hardcoded lineage with permutation convergence.** Drop
   `DIVERGENT_MIGRATIONS`/`CONVERGED_VERSIONS`. A ledger converges when:
   - every applied `version` parses to a release-scoped id, and its `_name`
     suffix names a migration in the canonical registry;
   - the multiset of applied names **equals** the set of names in a *leading
     prefix* of the registry (same migrations, same count, no unknown name, no
     gap); and
   - the strengthened product-schema fingerprint equals the fingerprint built by
     applying exactly that canonical prefix in order.

   When all three hold, rewrite the ledger to canonical: for the prefix's
   canonical name order, take the **sorted multiset of the existing `applied_at`
   values** and assign them ascending to the canonical order, then rewrite each
   row's `version` string accordingly. Sorting the timestamps (not preserving
   them per-name) is the crux the incident recovery proved: the live timestamps
   sort `profiles` before `context_pressure`, but canonical wants the reverse, so
   `(applied_at, rowid)` must be made to yield canonical order. No SQL is
   re-executed — the schema is already correct by fingerprint equality; only
   ledger identity/order changes. Idempotent: a canonical ledger matches the
   prefix trivially and rewrites to itself.

3. **History-fingerprinted backup.** Before the rewrite, name the backup by a
   hash of `(ordered applied version list + product-schema fingerprint)`, not by
   latest version alone. This closes the "stale/wrong-lineage backup reuse" gap
   (Why-2): two different histories with the same latest version can no longer
   collide on one backup filename, and a reused backup is proven to match the
   pre-repair state before it is trusted.

### PR 2 — Prevent recurrence at the authoring boundary

4. **Provenance + checksum columns.** New migration
   `0.11.015_migration_provenance` adds `checksum`, `build_provenance`,
   `source_identity`, `package_version` to `schema_migrations` (nullable;
   pre-existing rows backfill `checksum` from the known SQL, provenance `NULL`).
   Every `INSERT` into the ledger records `sha256(sql)` plus
   `build_info::provenance()/source_identity()` and `CARGO_PKG_VERSION`. The
   checksum is the collision-resistant durable identity that lets a same-ordinal
   name-conflict be *distinguished* (different content → different checksum →
   real conflict, correctly rejected) and lets convergence also require
   per-migration checksum equality once the column exists.

5. **Allocate against the integration branch.** `new_migration.py` fetches
   `origin/main` and computes the next ordinal from main's registry (union with
   the local worktree), so a branch cut from main no longer silently reuses
   main's next number.

6. **Convergence-matrix CI gate.** `check_migrations.py` gains a check that
   simulates converging merge-base, current main, and this PR's registry into
   the proposed merge result, and fails when the PR reuses an ordinal for a
   migration whose name/checksum differs from main's — catching the collision at
   PR time, the earliest durable-publication event we can gate.

7. **Doctrine.** Update `MIGRATIONS.md` next to the store: an ordinal is a
   sortable label, not an identity; identity is name + content checksum;
   reordering a known name-set self-heals; a name/checksum conflict is a real
   merge conflict a human resolves.

## End-to-end proof

- **Recovery (the incident):** a sanitized fixture under
  `rust/loopflow/tests/fixtures/` (or an in-test builder) reproduces the exact
  live ledger — rows `008_interactive_handoffs`, `009_profiles`,
  `010_provider_account_lifecycle`, `011_context_pressure`,
  `012_context_input_normalization` with the live `applied_at` shape and real
  product data in a few tables. Test: `apply_sqlite_with_backup` opens it,
  `latest_version_sqlite` returns `0.11.014_task_lifecycle`,
  `applied_versions` equals the canonical registry, `PRAGMA integrity_check`
  is `ok`, the seeded rows survive, and the backup file exists with the
  pre-repair fingerprint. This is the whole outage reproduced and closed.
- **Tolerance:** a property-style test permutes the ordinals of a known
  contiguous name-set and asserts every permutation converges to canonical
  with row counts preserved.
- **Prevention:** `check_migrations.py` gains a test where a synthetic PR
  registry reuses main's next ordinal for a different migration and the gate
  exits non-zero; `new_migration.py` gains a test that its ordinal clears
  main's registry.

## Affected surfaces and consumers

- `rust/loopflow/src/store/migrations.rs` — convergence, fingerprint, ledger
  writes, provenance (core).
- `rust/loopflow/src/store/migrations/0.11.015_migration_provenance.sql` — new.
- `rust/loopflow/src/store/sqlite.rs::SqliteStore::new` — unchanged call site;
  auto-repair rides the existing `apply_sqlite_with_backup` path.
- `rust/loopflow/src/lf/commands/doctor.rs` — surfaces provenance/checksum in
  its store report (already renders `BuildProvenance`); no behavior gate.
- `scripts/new_migration.py`, `scripts/check_migrations.py` — allocation +
  convergence gate.
- `MIGRATIONS.md` — doctrine.
- The `--json` shape of `lf doctor` gains provenance fields (additive; DTO rule:
  new fields are required or explicitly optional, added to fixtures).

## Absent / error states

- **Foreign / flat-ledger DB** (pre-loop `001_initial`, or product tables with
  no ledger): unchanged — still `RECREATE_MESSAGE`. Convergence never fires
  because names don't match the registry or the fingerprint diverges.
- **Genuinely edited shipped migration:** fingerprint (and, post-PR2, checksum)
  mismatch → still rejected. Convergence must never launder a real schema drift.
- **Newer-release ledger** (unknown release-scoped id whose name is not in this
  binary's registry): unchanged downgrade message.
- **Backup collision / IO failure:** the existing atomic temp-then-rename and
  exclusive-lock path is preserved; a failed backup aborts before any ledger
  rewrite.
- **Partial permutation** (some but not all of a prefix's names present, or an
  extra unknown name): does not converge → normal rejection. Convergence is
  all-or-nothing against a complete prefix.

## Operational boundary

Recovery runs inside the single exclusive migration transaction already held by
`apply_sqlite_with_backup`; it adds one in-memory schema build and a handful of
`UPDATE`s, negligible against the backup copy. No new subprocess or network on
the open path. `new_migration.py`'s `git fetch` is authoring-time only, never on
the hot path.

## Exclusions (filed as follow-up Tasks under this Project)

The directive scopes these out ("keep unrelated store decomposition out"):

- Telemetry-optional trace capture: make implicit internal-agent trace capture
  warn-and-continue when the ledger is unavailable (the acute `lf pr land`
  blocker).
- Store-free mechanical `lf pr land`: prove the mechanical path completes with an
  absent/incompatible store, degrading only copy generation.
- Store capability boundaries: audit every store opener as none/read-only/
  subsystem/full-schema.
- Separate databases for auth/profile vs. operational vs. telemetry.
- `lf store clone`: a supported production→branch dogfood snapshot so live
  dogfooding stops depending on the break-glass bypass.

## Review questions (Hashimoto lens)

- Can a foreign DB with a coincidentally-identical schema *and* identical
  migration names in a different order be adopted? Yes — but that database *is*
  ours by construction; there is no other way to produce it. The strengthened
  fingerprint is what makes "identical schema" a real guarantee.
- Does the generalized converger have a 2 a.m. failure that the hardcoded one
  didn't? It touches more inputs, but every rewrite is gated by full fingerprint
  equality and is a no-op when the ledger is already canonical; the backup is
  taken first and proven. The hardcoded lineage constants are deleted, not
  supplemented — one implementation, per CLAUDE.md.
