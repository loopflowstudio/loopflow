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
  floors, so released lf (≤0.15.0) still reads a store until the first slug
  migration lands; after that it fails to parse the slug row and prints its own
  "delete loopflow.db" refusal (questions.md).
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

## Concept review with Jack (2026-10-28) — Desktop and schedule questions moved to LOO-669

Mismatch: "installed Loopflow" is two programs. `lf self install --main`
replaces the CLI; Loopflow.app keeps running its bundled release lf
(`Contents/MacOS/lf`, `controlLfPath`) against the same `~/.lf`. Every
migration has been released by now (`0.15.0.001_release` is the last), so the
first slug migration that merges, followed by `install --main`, writes a ledger
row that released readers don't parse (questions.md). Desktop then refuses the
store until a release ships an app with the new reader. After that release, only
breaking slugs lock Desktop out. The laptop schedule (`lf install` LaunchAgent)
also still follows releases, so a scheduled refresh never moves to main.

Proposed (not accepted): an installed *channel*, either release or main, covering CLI and
app, which the schedule keeps following. Open questions, on `scratch/concept-review.html`:
1. How Desktop follows main: (a) `Packages (main)` builds the app and
   `install --main` replaces both, (b) the app runs the installed lf instead of its
   bundled helper, (c) Desktop stays on releases and a release goes out before the
   first slug migration and before each breaking one.
2. Whether the schedule follows the channel last installed.

Jack leans 1a ("probably", 2026-10-28); not yet final. Consequences recorded for
that choice:
- CI holds no Developer ID; `scripts/release-loopflow.py` signs with the local
  keychain. 1a needs the cert in CI (Doppler → GitHub secret → temporary
  keychain on a macOS job). That's outward-facing setup, so it goes in its own Task.
- Main packages likely need Developer ID signing but not notarization: Gatekeeper
  checks quarantined files, and the expectation is that a `gh` download unpacked
  by the installer carries no quarantine flag (unverified, check on a real
  download). Ad-hoc signing would reset TCC grants on every install.
- Alternative: `install --main` builds the app locally from the same commit
  with the keychain identity. No CI secrets, but slow, and only on Machines
  holding the cert.
- Proposed sequencing: 2a (the schedule follows the recorded channel) and 3b (merge this PR,
  cut a release right after so the app's reader admits additive slugs, then do
  1a as a follow-up Task). Awaiting Jack.

### Split with LOO-669 (agreed between the Task conversations, 2026-10-28; Jack asked for the division)

- LOO-669 owns streams (dev/latest/stable pointers), the recorded stream with
  schedule/self-update following it (Q2), how Desktop follows each stream including
  Developer ID signing for main packages (Q1a), minors from main and the release docs.
- LOO-656 owns migrations, `lf self install --main` as the dev-stream
  fetch-and-switch, the open-store predicate, and contract enforcement. LOO-656
  lands first. LOO-669 redoes only its minor-pairing removals on top, so
  LOO-656 doesn't touch `MinorRelease`, the closing patch, `prepared_tree`,
  `NeedsIntegration` or the fix-forward ban.
- **Predicate over published data.** Each package (main and release) carries a
  small catalog file listing the reader-breaking migration ids the build knows.
  LOO-656 generates it in `build.rs`/packaging and provides
  `can_open(store_applied, candidate_catalog)`. LOO-669 publishes the file next to
  `SHA256SUMS` for each release and stream pointer, and calls the predicate before
  downloading.
- **`CONTRACT_WAIT` = 30 days, one constant**, which must exceed stable lag (7-day
  soak + ≤ 1 day cadence). LOO-669's stream docs reference it.
- One ordinary release right after LOO-656 merges (a patch if minors-from-main
  isn't in yet).

### Top-3 pursue (Jack, 2026-10-28) — implemented 2026-10-28

1. Refusals keep the store: `incompatible()` names the lf that last migrated
   it or `lf self install --main`; the newer-schema refusal names
   `lf self install --main`. No message suggests deletion.
2. `swift/README.md` names `lf self install --main` and `lf install`;
   `MIGRATIONS.md` says a private `LF_HOME` starts empty.
3. `store::migrations::reader_catalog()` (released batches + breaking slugs)
   and `can_open(conn, catalog)` (catalog holds the floor of the store's newest
   row). `lf self install reader-catalog` prints it; `package-build.yml` ships
   `reader-catalog.txt` in each `lf-<target>` artifact. LOO-669 publishes it next
   to `SHA256SUMS` and calls `can_open`; nothing in LOO-656 calls it yet.

### Concept review of the pursue (headless, 2026-10-28) — implemented

Jack asked for "refusals that name the build to install". The newer-schema
refusal does: it names the floor migration and `lf self install --main`.
`incompatible()` only says "the lf that last migrated it", but the ledger already
holds that build: every row a provenance-aware lf wrote carries
`package_version` and `source_revision`. Settled by the accepted request:

- `incompatible(conn)` names the build from the newest ledger row via
  `last_migrated_by` ("last migrated by lf 0.14.2 (012345678)"; an `unknown`
  revision prints the version alone). A ledger without provenance columns fails
  that query and keeps the generic wording. `pending_migrations` takes the
  connection so every refusal path names the build. No new state.
- Test `a_drifted_store_names_the_build_that_last_migrated_it`.

Reviewed and unchanged: `reader_catalog`/`can_open` match the LOO-669 split
(`can_open` has no caller until LOO-669 publishes catalogs, as agreed), and
`lf self install reader-catalog` stays a hidden packaging command. The docs
changes match the request. No open product choice.

## Not in this change: contract enforcement (decision belongs to LOO-669)

Jack, 2026-10-28, as relayed from LOO-669: "i also declined contract enforcement
as a separate PR. I just want to decide here in this task." This PR ships no
`CONTRACT_WAIT` and no `-- contract:` header. A breaking slug keeps raising the
floor; install prints which migration locks older builds out, and the reader
catalog makes it visible to updaters.

## Remaining work

- Refresh the PR description at publish: its risk line names ≤0.14.19 where
  released lf ≤0.15.0 is meant, `copy:` is gone from `lf --version`,
  stream/Desktop follow-through belongs to LOO-669, refusals no longer suggest
  deletion, and every package ships `reader-catalog.txt`.
- First real `lf self install --main` needs a green `Packages (main)` run after merge.

Check (2026-10-28, realign): `cargo test -p loopflow --lib store::migrations::` 121 passed at 9fa92571b, `cargo fmt --check` clean; `lf self install reader-catalog` with empty `HOME`/`LF_HOME` prints every released batch and creates nothing; full `store::` module (times out locally), Swift and `release_tests` left to gate/CI.
