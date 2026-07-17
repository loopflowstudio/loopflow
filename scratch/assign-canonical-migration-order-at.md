# Assign canonical migration order at the release cut

## Problem

The migration ordinal is allocated while a branch is active. `scripts/new_migration.py`
fetches `origin/main`, counts the migrations in the active `<major>.<minor>` namespace,
and writes the next `<ordinal>`. Two consequences follow, both live on main right now:

1. **Ordinals race.** Two branches authored concurrently both pick the same next
   ordinal; whichever merges second must rebase and renumber. Independent schema
   changes coordinate on a serial number instead of on their real data dependency.
   `check_migrations.py` with `LOOPFLOW_REQUIRE_ORIGIN_MAIN=1` then turns
   `migration-check` red on *every* branch that is merely behind main, whether or
   not it touches a migration — a recurring source of manual git surgery
   (wave `MEMORY.md`: "Migration ordinals race until merge").

2. **`origin/main` is treated as the publication boundary because dogfooding runs
   ahead of tags.** So a migration is "canonical" the instant it merges, not when it
   is released. Main currently holds `0.11.027_accounts_first` then
   `0.11.029_ci_incident_repaired_head`; `0.11.028_task_gate_settlement` is still
   isolated on open PR #1052. If #1052 lands after a main build has already applied
   029, the canonical source order and a live ledger disagree even though each branch
   was locally reasonable. The `lf code-review` breakage on 2026-07-17 was the same
   class from the promotion side (W2-319 owns that half).

Who benefits: every developer and agent authoring schema in parallel. The win is
that parallel schema work never contends for, reserves, or renames an ordinal, and
"which migration comes first" is decided once, deterministically, at the one moment
that is actually a publication — the release cut.

## The demo

```
# two branches, authored the same afternoon, no coordination:
uv run python scripts/new_migration.py add_wave_colour      # branch A
uv run python scripts/new_migration.py add_task_priority    # branch B
# each writes a draft with a stable name and NO ordinal; no fetch, no rebase race.
# both merge to main in either order — migration-check stays green on both.

lf release run patch
# the release worktree orders the accumulated drafts, assigns the next canonical
# ordinals in one contiguous tail, proves the previous release upgrades cleanly
# through them, opens the release PR, merges it, then tags that exact commit.
# Re-running `lf release run patch` after a failed attempt regenerates byte-identical
# canonical files.
```

The moment that proves the win: `git log --stat` on the two merged feature branches
shows `drafts/add_wave_colour.sql` and `drafts/add_task_priority.sql` — no ordinal,
no collision — and `git show <release-tag>` shows them canonicalized as a contiguous
`0.11.030_… / 0.11.031_…` tail assigned in one deterministic step.

## Approach

Split migration identity into two lifecycle stages that share one model:

- **Draft** — authored on a branch. Lives at
  `rust/loopflow/src/store/migrations/drafts/<name>.sql`, registered in a `DRAFTS`
  slice beside `MIGRATIONS`. Carries a **stable name** and an optional
  **`depends_on`** list of other draft names. It has **no ordinal**. Drafts are
  mutable and applied only to disposable development/test databases.

- **Canonical** — a released migration, exactly today's `Migration` /
  `<major>.<minor>.<ordinal>_<name>.sql`. Immutable once merged. The *only* way a
  draft becomes canonical is the release cut.

Canonicalization becomes a step inside the existing `lf release run` worktree, before
the migration gate and the PR: it freezes the draft set, validates the dependency
graph, topologically orders it, assigns the next ordinals in the version-being-cut's
namespace, rewrites each draft into a canonical `.sql` + `MIGRATIONS` entry, deletes
the draft, and proves the previous release upgrades through the new tail. This reuses
`lf release run`'s existing worktree/PR/merge/tag machinery rather than adding a second
release mechanism.

The development-store fence already exists and is reused unchanged:
`MigrationAuthority::{Published, ValidationOnly}`, `guard_development_database`, and
`may_apply_migrations` (`store/mod.rs`) already refuse a dev build against
`~/.lf/loopflow.db` and give each source worktree its own
`~/.lf-dev/worktrees/<source-identity>/loopflow.db`. Drafts only ever exist in a
development build (a released binary ships an empty `DRAFTS`), so they can never reach
the shared release store. This Task adds no new fence; it relies on W2-319's.

### Draft file format

```sql
-- name: add_wave_colour
-- depends_on: add_wave_table, seed_wave_defaults
ALTER TABLE waves ADD COLUMN colour TEXT;
```

- `name`: snake_case, unique across all drafts and all released migration names.
  This is the draft's whole identity. It survives canonicalization unchanged — the
  canonical file is `<id>_<name>.sql` — so any test or helper keyed on the name keeps
  working across the rename.
- `depends_on`: comma-separated draft names, or omitted / `none`. Names **other
  drafts only**; released migrations already precede every draft by construction, so a
  draft never depends on a canonical id. A data migration that must run after another
  draft declares it here — this is the dependency graph, never the ordinal.

### `DRAFTS` registry

Beside `MIGRATIONS` in `migrations.rs` (or a sibling `drafts.rs`):

```rust
pub struct DraftMigration {
    pub name: &'static str,
    pub depends_on: &'static [&'static str],
    pub sql: &'static str,
}

const DRAFTS: &[DraftMigration] = &[
    DraftMigration {
        name: "add_wave_colour",
        depends_on: &[],
        sql: include_str!("migrations/drafts/add_wave_colour.sql"),
    },
];
```

`new_migration.py` writes the draft file and prints this entry to paste — the same
"write SQL, paste the printed entry" ergonomics as today, minus the ordinal.

### Development / test application

The store applies the immutable canonical chain, then overlays drafts:

1. Apply `MIGRATIONS` exactly as today (incremental, immutable prefix).
2. Topologically sort `DRAFTS` (dependency edges; ties broken by name — deterministic,
   never by file order, PR number, or time). Apply each pending draft, recording it in
   `schema_migrations` under version `draft.<name>` with its SQL checksum.
3. On open, reconcile the draft overlay against the current `DRAFTS`: any recorded
   `draft.*` row whose draft was removed, renamed, or whose checksum changed means the
   overlay is stale. Because drafts are mutable and the dev store is disposable, the
   response is to **recreate the development store** (back up, delete, re-init canonical
   + current drafts fresh) and log it loudly. Appending a new draft with the canonical
   prefix unchanged just applies the new draft; a *changed* draft recreates.

A released build ships `DRAFTS == &[]`, so it never applies or records a `draft.*` row.
Combined with `may_apply_migrations`, `draft.*` rows exist only in development stores,
and a released binary that encounters one treats it as an unknown id (incompatible),
exactly as it should.

### Release canonicalization (`scripts/canonicalize_migrations.py`, stdlib-only)

Run by `lf release run` in the release worktree, before `verify_migrations`:

1. Read `DRAFTS` and the draft files. Validate: names unique and snake_case; no
   collision with any released id or name; every `depends_on` resolves to a draft; the
   graph is acyclic.
2. Topologically sort (Kahn), tie-breaking ready nodes by name → a total, deterministic
   order independent of merge timing.
3. Choose the namespace from the **version being cut** (`resolve_version`): patch keeps
   the current `(major, minor)` and continues after the last released ordinal in it; a
   minor/major bump starts a fresh `(major, minor)` sequence at ordinal 1.
4. Assign a contiguous ordinal tail. For each draft in order, write
   `migrations/<major>.<minor>.<NNN>_<name>.sql` (draft SQL body, header stripped),
   delete `drafts/<name>.sql`, append the `Migration { .. }` entry to `MIGRATIONS`, and
   remove the `DRAFTS` entry.
5. Idempotent + deterministic: re-running on an empty `DRAFTS` is a no-op; the same
   draft set always yields the same ids, files, and diff. A failed/abandoned release
   never merged its ids, so a later release regenerates them identically.

### The previous-release upgrade gate

A committed fixture proves data transforms before publication:
`rust/loopflow/tests/fixtures/migrations/previous_release.db` holds the **last
released** schema frontier with representative product rows.

A `#[test]` (so it runs on every `cargo test` *and* in the release PR's CI, which is
before the tag and before any shared-store write) copies the fixture, applies the full
current `MIGRATIONS` chain, and asserts: no error, `PRAGMA foreign_key_check` clean,
product schema byte-identical to a fresh init, checksums valid, and a second open is a
no-op. A fresh-init proof and the no-op-reopen already exist in `migrations.rs` tests.

The release worktree also **regenerates** `previous_release.db` to the new frontier
(open fresh, apply the now-canonical chain, apply the committed seed rows, snapshot) and
commits it in the release PR, so the next release's fixture sits at this release's
frontier. At gate time the fixture is still the prior frontier — exactly "a fixture at
the previous released frontier."

Per-migration data-transform correctness is a normal `#[test]` using a name-keyed
helper `apply_through(conn, "add_wave_colour")` that applies canonical chain + drafts up
to and including the named migration. The helper resolves a name whether it is currently
a draft or already canonical, so the test is written once and survives the release
rename — the data logic is never untested until release.

## De-risking

| Question | Finding | Impact on design |
|----------|---------|-----------------|
| Does a dev/prod store fence already exist, or must this Task build one? | Yes: `MigrationAuthority`, `guard_development_database`, `may_apply_migrations`, and per-source `~/.lf-dev/worktrees/<source-identity>/loopflow.db` all exist (`store/mod.rs`, `build_info.rs`). W2-319 owns promotion safety. | Reuse it verbatim. Drafts live only in dev builds (empty `DRAFTS` in release), so no new fence — satisfies "fail closed against the shared release store" and the exclusion "do not weaken W2-319's fence." |
| Can canonicalization reuse `lf release run`, or is a second mechanism needed? | `release_run` already creates a dedicated worktree, bumps manifests, writes notes, commits, opens the PR, waits for merge, then tags the merged commit (`ops/release.rs`). `verify_migrations` runs `check_migrations.py` in that worktree. | Add canonicalization as one more `prepare_release_in_worktree` step before `verify_migrations`. No new release path. |
| Will `apply_set` accept a synthetic release history for the upgrade fixture? | `apply_set(conn, set: &[Migration])` already takes the set as an argument precisely so "fixtures can drive synthetic release histories without a test-only seam." Existing divergent/permuted tests do this. | The upgrade gate and per-migration tests use `apply_set` on slices; no production reshaping for tests. |
| Does the file stem have to equal the ledger version string? | Yes — `Migration::version()` is `format!("{id}_{name}")` and is exactly the file stem; renaming a shipped file is a schema break the immutability check enforces. | Canonicalization writes `<id>_<name>.sql` with the draft's `name` unchanged, so the recorded version is stable; the name is chosen at authoring time and never revised at release. |
| How do drafts avoid becoming a second canonical namespace (the design-pressure ban)? | Drafts carry no ordinal and record `draft.<name>` (which `MigrationId::parse_version` rejects as a release id). They are mutable, dev-only, and deleted at canonicalization. | There is exactly one canonical namespace (`MIGRATIONS`); `DRAFTS` is a staging area that empties at every release, not a parallel quasi-canonical order. |
| Does `check_migrations.py`'s `origin/main` staleness failure survive? | It fails a branch only when a *canonical* migration disagrees with `origin/main`. Feature branches now add drafts, not canonical ids. | The "X exists on origin/main but missing from this branch" red on merely-behind branches disappears for schema-adding branches — a direct developer-efficiency win. The check gains draft validation (names, graph) but drops branch-ordinal collisions. |
| What decides order between two independent drafts (no dependency)? | Topological sort with a name tie-break is total and deterministic; merge time / PR number / file mtime are not reproducible on release retry. | Ordinal assignment is reproducible: same drafts → same ids. Satisfies "re-running release yields no migration diff" and "do not infer order from filename sort/PR/time when a dependency exists." |
| Is recreating the dev store on draft drift acceptable? | The dev store is explicitly disposable (`~/.lf-dev/...`), and this repo dogfoods on the *release* binary against `~/.lf/loopflow.db`; dev builds are walled off. | Recreate-on-draft-change is sanctioned by the Task ("a schema frontier change recreates that development database"). Back up before delete; never touch the release store. |

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Central ordinal reservation service (a store that hands out the next number) | Removes the race but adds a networked coordination point and a new failure mode; ordinals still allocated pre-release. | Explicitly banned by the Task's design pressure; heavier than the problem. |
| Keep branch ordinals but renumber deterministically at rebase | No new file format. | Still allocates on branches, still renames on rebase, still coordinates on a serial number — exactly the behavior the Task removes. Two quasi-canonical namespaces. |
| Content-hash draft identity (`draft_<sha8>.sql`) instead of a human name | No name collisions to police. | Opaque; the Task asks for "a content/task identity **plus an explicit human-readable name**." A name is also what lets `apply_through` and tests survive the rename. |
| Order drafts by filename/PR at release | Trivial to implement. | Not a dependency graph; a real data dependency between two drafts would be silently mis-ordered. The Task forbids inferring order from filename/PR/time when a dependency exists. |
| Regenerate `previous_release.db` after the tag instead of in the PR | Simpler sequencing. | The gate needs the fixture at the *prior* frontier at PR time; regenerating post-tag leaves the next release with no committed fixture. Regenerate in the PR (fixture still prior-frontier at gate time). |

## Key decisions

- **Draft name is the identity and it never changes at release.** The canonical file is
  `<id>_<name>.sql`; only the `<id>` prefix is minted at the cut. This is what keeps
  name-keyed tests and `apply_through` valid across the rename and keeps the recorded
  `schema_migrations.version` stable.
- **Canonicalization is Python (`scripts/canonicalize_migrations.py`), stdlib-only**,
  matching `new_migration.py` / `check_migrations.py`, so `lf release` needs no toolchain
  to canonicalize. The upgrade *gate* is Rust (`#[test]`) because it needs the migration
  engine (checksums, schema compare); it runs in the release PR's CI before the tag.
- **Drafts are dev-only by construction.** A released binary ships `DRAFTS == &[]`;
  combined with `may_apply_migrations`, `draft.*` ledger rows exist only in development
  stores. No new fence, no production risk.
- **Recreate the dev store on draft drift**, never advance it across a mutated draft —
  drafts are not immutable, so incremental application across an edit is unsound.
- **Topological order with a name tie-break**, computed from `depends_on`, is the single
  source of migration order for the release tail. Deterministic and retry-stable.
- **`check_migrations.py` gains draft validation and loses branch-ordinal collision as a
  routine failure.** It validates draft names/graph and keeps every canonical
  immutability check (vs `origin/main`, vs last tag) unchanged.

## Scope

**In scope**

- `drafts/` directory, draft file format, `DRAFTS` registry, `DraftMigration` type.
- `new_migration.py` rewritten to emit a draft (no ordinal, no fetch/rebase).
- Dev/test store application of the draft overlay + recreate-on-drift.
- `scripts/canonicalize_migrations.py` and its wiring into `release_run` before
  `verify_migrations`.
- `check_migrations.py` draft validation; retain all canonical immutability checks.
- The previous-release upgrade fixture, its gate `#[test]`, fixture regeneration in the
  release worktree, and the `apply_through` name-keyed test helper.
- `MIGRATIONS.md`, doctor output, and CI `migration-check` updated to describe the
  release cut as the publication boundary.

**Out of scope**

- Weakening or duplicating W2-319's shared-store promotion fence.
- Making production databases disposable.
- Retroactively renaming released migrations.
- Repairing the live 028/029 history (separate operational cleanup — see below).

### Forward-compatible transition for 028/029

This Task prevents recurrence; it does not rewrite history. `0.11.027` and `0.11.029`
stay canonical as-is. `0.11.028_task_gate_settlement` on PR #1052 is reconciled by the
operator by either landing it with its current ordinal (a one-time gap in the sequence,
which the numeric-tuple ordering tolerates) or re-authoring it as a draft to be
canonicalized at the next release. The design leaves both ordinals immutable and adds no
code that renumbers them.

## Done when

- `scripts/new_migration.py add_x` writes `drafts/x.sql` with no ordinal and performs
  no `git fetch`/rebase; `test_new_migration.py` asserts this.
- Two branches each add a draft; both merge in either order; `check_migrations.py`
  (with `LOOPFLOW_REQUIRE_ORIGIN_MAIN=1`) passes on both — no rename, no collision.
- `scripts/canonicalize_migrations.py` on a two-draft set with a declared dependency
  emits a contiguous ordinal tail in dependency order; on a cycle it fails before any
  file is written; re-running yields a byte-identical result.
- A Rust gate `#[test]` upgrades `previous_release.db` through the full chain
  (FK-clean, schema == fresh, checksums valid, no-op reopen) and proves fresh init.
- `apply_through("<name>")` applies canonical chain + drafts up to a named migration,
  used by a data-transform test that passes both before (draft) and after (canonical)
  the rename.
- A dev store with a changed draft recreates rather than advancing; a released build
  (empty `DRAFTS`) never writes a `draft.*` row and refuses `~/.lf/loopflow.db`.
- `cargo test -p loopflow`, `cargo fmt`, `cargo clippy -- -D warnings`,
  `uv run pytest python/tests/test_new_migration.py python/tests/test_check_migrations.py`,
  and a new `test_canonicalize_migrations.py` all pass.
- `MIGRATIONS.md` and the CI `migration-check` comment describe the release cut, not
  `origin/main`, as the publication boundary.

## Measure

- **Avoidable `migration-check` reds** — the count of branches failing `migration-check`
  solely because they are behind main while adding a migration. Baseline from
  wave `MEMORY.md`: this fires on effectively every schema-adding branch and on
  behind-main branches that touch no migration. Target after: **zero** for
  schema-adding branches (they add drafts, no ordinal, no `origin/main` collision).
- **Ordinal renames per merged schema PR** — `git log` count of migrations renamed
  during rebase. Baseline: nonzero and rising with parallelism (027/028/029 is the live
  instance). Target: **zero** — ordinals are assigned once, at the cut.
- **Release-retry determinism** — re-running `lf release run <bump>` after an aborted
  attempt produces an empty migration diff. Target: **byte-identical** canonical files.
