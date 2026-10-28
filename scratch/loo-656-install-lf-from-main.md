# LOO-656: install lf from main

Status: accepted direction (Jack, 2026-10-27, Task description); implementation
decisions below were made headless during `implement` and are recorded in
`questions.md` where reversible.

## Outcome

`lf self install --main` installs the CI build of the newest green main commit and
migrates `~/.lf` forward. A worktree build whose migrations the store already has
runs its own code on the live store. Main never carries a "draft".

## Model

- **Released history is frozen.** `migration_catalog.rs` keeps the
  `<ver>.<NNN>_release` entries and their reader floors byte for byte. Nothing
  appends to it again.
- **New migrations are `src/store/migrations/<slug>.sql`.** The slug, chosen in
  the PR, is the ledger `version`. One per Task, edited in place until merge.
- **Order is main's first-parent history**, computed by `build.rs`:
  `git log origin/main --first-parent --diff-merges=first-parent --diff-filter=A`
  over the migrations directory. Files in the tree that `origin/main` never added
  are **unmerged**; they sort after merged ones by name. Every build of a main
  commit computes the same order, so ids exist only for bytes on origin/main.
- **Breaking is proven, not declared.** `build.rs` applies each slug migration over
  the schema before it (the released chain, then earlier slugs) and proves it
  additive: old objects unchanged, tables only gain nullable/defaulted columns,
  new objects are tables or non-unique indexes, no statement writes an existing
  table. `-- breaks: <reason>` forces breaking.
- **Floors are membership.** Each slug migration's floor is the version of the
  newest breaking migration at or before it (the last released migration seeds
  it). The ledger's `reader_floor` stores it. A binary admits a store holding
  unknown rows only when every unknown row's floor is a version it knows, and the
  objects it knows survive (`validate_schema_within`). Released rows keep release
  floors, so ≤0.14.19 still reads a store until the first slug migration lands.
- **Who migrates `~/.lf/loopflow.db`.** Only the install switch
  (`open_as_promotion_boundary`, under the promotion lock, after the in-transaction
  backup). It refuses unmerged migrations. Ordinary opens validate and refuse a
  pending migration with the fix (`lf self install --main`, or `LF_HOME` for
  unmerged schema work). Any other path (`LF_HOME`) migrates itself, unmerged
  included; a changed unmerged migration is a checksum mismatch that names a fresh
  `LF_HOME` as the fix.
- **No re-exec.** A build runs its own code on whatever home it selects.
- **Install trust comes from source.** `decide` no longer reads a build-time
  authority; it refuses unmerged pending migrations and incompatible stores.
- **Per-green-main packages.** `packages-main.yml` runs after CI succeeds on main
  and uploads `lf-<target>` artifacts; it replaces the nightly. `lf self install
  --main` downloads the newest successful run's artifact with `gh`, then runs the
  candidate's `install promote` (CLI only; the app still ships with releases).

## Delete — do not maintain

- Copy-on-draft: `store/copy.rs` except `snapshot()`; `copy::enter`/`selected`,
  `~/.lf/copies/`, disk-cleanup of copies.
- `lf clone`: `data_clone.rs`, `tests/data_clones.rs`, `enter_selected`,
  read-only-session guards, Swift `LF_CLONE_ROOT`, `data_clones::` CI steps.
- Drafts: `migration_drafts.rs`, embedded draft manifest, `draft_schema.json`,
  `pending_migration_drafts`, Custom-Machine exact-schema rule
  (`initialize_experimental_sqlite`, `validate_experimental_*`,
  `IncompatibleDevelopment`), `--materialize-for-tests` +
  `scripts/materialize_rust_tests.py`, release-cut canonicalization
  (`scripts/canonicalize_migrations.py`, `lf release run`'s cut, BREAKING.md
  generation from floors).
- Authority split: `MigrationAuthority`, `LOOPFLOW_MIGRATION_AUTHORITY`,
  validation-only refusals in `decide`/`may_apply_migrations`.
- `installation::dispatch_default_cli` re-exec to the installed lf.
- Nightly packages workflow (replaced by per-green-main packages).
- Publisher `inspect` stage and the release-run repair loop (deleted in compress).
  `preparation_missing` stays: it tracks the publisher's signed `candidate.json`,
  not migrations.

Keep: `LF_HOME` isolation, lf-world, released-schema seed fixtures, reader-floor
tests, the Docker upgrade test, per-migration FK/JSON checks, backups.

## Concept review (headless, 2026-10-27)

Three user actions, three concepts: merge a PR (slug migration gets its place in
main's order), `lf self install --main` (the only path that advances
`~/.lf`), run a worktree build (opens `~/.lf` when it knows nothing the store
lacks; otherwise names `install --main` or a private `LF_HOME`). The accepted
decisions settle every name and boundary here; no open product choice.

Settled by the accepted contract, recorded so later steps don't relitigate:

- **One build per foreground command, installed build for what outlives it.**
  A worktree build's own process runs its code; Graph steps and agents' nested
  `lf` resolve the installed entry. Those run detached for hours, and a
  `cargo build` or a removed worktree would swap or delete their binary mid-run
  (Jack: "I have LF processes running 24/7"). Floors keep the mix safe: the
  installed build is never behind the store's merged migrations.
- **Post-merge window.** Between merge and the green `Packages (main)` run, a
  build rebased onto the merge is refused on `~/.lf`, and `install --main`
  still finds the previous green commit. Wait for the run; the refusal and the
  install output already name the migration and the installed commit.

## Not in this change (Jack: design still open)

Expand → wait 30 days → contract enforcement for destructive changes. Today a
breaking slug migration simply raises the floor; install prints which migration
locks older builds out.

## Remaining work

- Expand → wait → contract enforcement (design open).
- First real `lf self install --main` needs a green `Packages (main)` run after merge.

Check (2026-10-27): focused Rust store/install/release modules, clippy `--all-targets`, Python migration/release/install tests pass; three canonical-ids tests and one contention test fail only in the full parallel module run (pass alone); Swift and `release_tests` run left to gate/CI.
