# Complete release-boundary migration canonicalization (ENG-24)

## Problem

ENG-23 (#1076) split migration authoring from ordinal assignment: authors write
an ordinal-free **draft** and the release cut is supposed to freeze the accumulated
drafts into canonical, ordered, ordinal-assigned migrations. The half that shipped
is the *filesystem rewriter* (`scripts/canonicalize_migrations.py`) and its docs.
The half that makes it a real release boundary did not ship. Concretely:

1. **Nothing invokes canonicalization.** `lf release run` (`ops/release.rs`) never
   calls the script. The release cut still ships whatever drafts happen to be on
   disk *as drafts* — they never become canonical migrations, so a release
   produces no runnable schema change. The docs describe an integration that does
   not exist.
2. **Draft registration has no executable path.** `new_migration.py` prints an
   instruction to paste a `DraftMigration { .. }` entry into a `DRAFTS` slice in
   `migrations.rs`. Neither the type nor the slice exists. Worse, that slice would
   be a **shared registry edit** — the exact merge-contention ENG-23 set out to
   kill, reintroduced one layer down.
3. **Draft identity is a bare human name.** Two branches that both run
   `new_migration.py add_wave_colour` write the same path `drafts/add_wave_colour.sql`
   and collide on merge. Same-name concurrency is not merge-independent.
4. **Cross-release-boundary dependencies are rejected.** `_order` fails a
   `--depends-on X` unless `X` is in the *current* frozen draft set. Once `X` was
   canonicalized in an earlier release, a later draft can no longer declare it as
   an upstream — the dependency vanishes at exactly the boundary it needs to cross.
5. **Canonicalization is not atomic.** It writes canonical files and unlinks drafts
   as it goes, edits the registry last. A write, graph, or registry failure partway
   through leaves a half-canonicalized tree, not a byte-identical one.
6. **Nothing proves a real migration.** The Python tests only exercise filesystem
   rewriting. No test migrates a real previous-release database through the
   generated canonical tail, proves fresh initialization, or requires Rust CI on
   the generated `migrations.rs` before the release merges/tags.

## The demo

```bash
# Two developers, two branches, same idea — no contention, no shared edit:
uv run python scripts/new_migration.py add_wave_colour   # branch A -> drafts/add_wave_colour__<idA>.sql
uv run python scripts/new_migration.py add_wave_colour   # branch B -> drafts/add_wave_colour__<idB>.sql
git diff --stat rust/loopflow/src/store/migrations.rs     # empty — no registry was touched

# The release cut is the only thing that assigns ordinals and installs them:
lf release run patch
#   Bumping manifests for v0.11.4...
#   Canonicalizing 2 draft migration(s) into 0.11...
#     0.11.004_add_wave_colour  <- draft add_wave_colour [ (none) ]
#     0.11.005_backfill_colour  <- draft backfill_colour [ add_wave_colour ]
#   Committing release changes...        <- canonical files + registry are IN this commit
# The release PR carries the new canonical migrations; Rust CI compiles and runs
# them against a real two-generation fixture before the queue merges and tags.
```

The win: a draft is merge-independent end to end (no shared file, no shared registry
edit), and the *only* thing that turns drafts into runnable, ordinal-assigned schema
is the release PR itself — proven by CI running the tail against a real database.

## Approach

Five coordinated changes; the canonicalization logic stays in the stdlib-only Python
script (release path needs an interpreter, not a toolchain — a deliberate ENG-23
property) and `lf release run` becomes the authority that invokes it.

### 1. Immutable draft identity + generated registration (findings 2, 3)

A draft is `drafts/<name>__<id>.sql`, where `<id>` is an 8-hex-char token minted at
authoring (`secrets.token_hex(4)`) and recorded in an `-- id:` header. The token is
the immutable identity; the snake_case `<name>` is the human-readable label and the
`--depends-on` handle. Two branches authoring the same readable name mint different
tokens, so they write different files and never collide or share an edit.

**A draft has no Rust registry entry at all.** Its *file's presence* is its
registration — canonicalization already discovers drafts by scanning the directory.
`new_migration.py` stops printing the bogus `DraftMigration` block; nothing edits
`migrations.rs` until the release cut appends the *canonical* `Migration` entries it
generates. This is what makes "two branches, no shared registry edit" true by
construction rather than by discipline.

Draft names are unique *within one release cut*: two drafts sharing a readable name
is a canonicalization-time error (rare, clear, human-resolved by renaming one draft),
which keeps `--depends-on <name>` unambiguous and the canonical `..._<name>.sql`
filenames clean. Concurrency safety is at authoring (distinct tokens); a same-name
clash only ever surfaces as one release-time validation failure, never a merge
conflict.

### 2. Wire canonicalization into the release worktree (finding 1)

`prepare_release_in_worktree` gains a `canonicalize_migrations` stage **after
`bump_manifest_versions` and before `commit_workflow`**, mirroring the existing
`verify_migrations` shell-out:

```
bump_manifest_versions(...)          // version is now known
canonicalize_migrations(wt_path, &version)   // NEW: drafts -> canonical, in the tree
run_release_notes_stage(...)
commit_workflow(...)                 // canonical files + registry are committed here
land(...)
```

Because canonicalization runs *before* the commit, the generated `migrations.rs` and
`.sql` files are part of the release PR, so real Rust CI compiles and tests them
before the merge queue merges and tags. The release run — not a human remembering to
run a script — is the publication authority. The standalone script remains only as a
`--check` preview for developers ("what will the next cut look like"); it installs
nothing that the release run does not.

### 3. Dependencies across release boundaries (finding 4)

`_order` (and `new_migration.py`'s `--depends-on` validation) resolves a dependency
against the union of the frozen draft set **and already-released migration names**:

- names a current draft → a real ordering edge;
- names a released migration → already ordered before every draft, so the constraint
  is satisfied with no edge (indegree unaffected);
- names neither → the only failure.

A released upstream is an ancestor of the whole cut, so it correctly imposes no
in-cut ordering while still being a legal, documented dependency.

### 4. Validate-then-atomically-install (finding 5)

Canonicalization splits cleanly into a **pure planning phase** and an **install
phase**:

- *Plan* (no writes): read + parse drafts, resolve cross-boundary deps, detect
  self/missing/cyclic deps and duplicate names, topologically order, assign
  contiguous ordinals, and build the full result in memory — every canonical
  `(path, bytes)`, the complete new `migrations.rs` text, and the list of draft
  files to delete. Any error here happens before a single byte is written, so the
  tree is trivially byte-identical.
- *Install* (writes, ordered for rollback): write canonical `.sql` files (tracking
  each created path) → replace `migrations.rs` via temp-file `os.replace` (atomic) →
  and only after both succeed, unlink the drafts. On any exception during install,
  roll back: delete the canonical files created so far and restore the original
  `migrations.rs` bytes; drafts are never touched until the very end, so a failure
  leaves them intact. Result: success installs everything; any failure leaves the
  tree byte-for-byte as it was.

### 5. Real migration proof + two-generation fixtures (finding 6)

- **Rust store tests** (they exercise the real `apply_sqlite` runner):
  - *Upgrade*: initialize a database at a **prior generation** (apply
    `MIGRATIONS[..k]`, exactly what a store that stopped at release N-1 looks like),
    then apply the full current set and assert the tail upgrades it cleanly to
    `latest_known_version` with foreign keys intact.
  - *Fresh init*: apply the full set to an empty database and assert it reaches the
    same schema. (Extends the existing `fresh_on_disk_database_reopens_at_the_latest_version`.)
  - The two-generation prior/target fixtures live under `tests/` (never `scratch/`),
    so `lf pr land`'s scratch-clear cannot strip them — the release PR retains its
    prior input fixture.
- **Rust release-integration test**: drive the release worktree stage with a draft
  present and assert the committed tree contains the canonical migration + registry
  entry and no draft — proving the *release path*, not a manual invocation, performs
  canonicalization.
- **Python tests** keep proving canonicalization output shape/determinism and add
  the new boundaries below.

## De-risking

| Question | Finding | Impact on design |
|----------|---------|-----------------|
| Does keeping logic in Python still make the release run the "only authority"? | The authority is *what triggers the write in a real cut*. Release run invokes it unconditionally before commit; the script's manual mode is a `--check` preview. A Rust integration test proves the release path does the install. | Keep stdlib Python (no-toolchain property preserved), wire from Rust. |
| Can a draft need a Rust presence before release? | No. Drafts are unreleased; they never run against a DB until canonicalized into `MIGRATIONS`. | Drafts are file-only — deleting the `DRAFTS`-slice instruction loses nothing and kills the shared edit. |
| Is a random token still deterministic for "aborted release regenerates identically"? | The token is minted once at authoring and persisted in the filename; canonicalization consumes it, never regenerates it, and it never appears in canonical output (only name+ordinal). | Canonical files/registry depend only on (names, deps, version) — still fully deterministic. |
| Does appending migrations break W2-319 promotion safety? | W2-319's install preflight requires the candidate binary to know migrations ≥ the store frontier; canonicalization only *appends* and never edits shipped migrations, raising `latest_known_version` monotonically. | Preserved: no change to `lf install` promote logic; ordinals stay monotonic and contiguous. |
| Can two drafts in one cut share a readable name? | Distinct tokens keep files distinct at authoring, but canonical `..._<name>.sql` and `--depends-on <name>` need name uniqueness within the cut. | Duplicate readable name in the frozen set is a canonicalization-time failure, not a silent pick. |

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Port canonicalization into Rust; delete the script | Single authority, no shell-out | Duplicates well-tested topo logic, and breaks the deliberate "release needs an interpreter, not a toolchain" property; the `verify_migrations` precedent already shells out to `python3`. |
| Keep drafts registered in a Rust `DRAFTS` slice (build the type ENG-23 named) | Rust sees drafts at compile time | It *is* the shared registry edit that reintroduces merge contention — the whole defect. Drafts need no Rust presence. |
| Content-hash as draft identity | No minted token | Draft SQL mutates during authoring, so a content hash is not stable; identity must be immutable from creation. |
| Gate canonicalization writes behind a release-only env var | Hard "only release path" enforcement | Over-engineering; the release integration test proves the wiring, and a manual `--check`/write is a legitimate dev preview, not a competing authority. |

## Key decisions

- **Draft identity = minted token; readable name stays for humans and `--depends-on`.**
  Immutability comes from minting-once, not from hashing mutable content.
- **Drafts are file-only; no Rust registry entry until canonicalization.** This is the
  single change that makes "no shared registry edit" structural.
- **Canonicalization stays stdlib Python, invoked by `lf release run` before commit.**
  Preserves the no-toolchain release property and gets the generated `migrations.rs`
  under real Rust CI on the release PR.
- **Plan/install split with rollback** for byte-identical-on-failure.
- **Real migrations are proven by the Rust store runner against a two-generation
  fixture**, because the Python layer cannot run the compiled migration path.

## Scope

- In scope: `new_migration.py` (token + generated registration + released-dep
  validation), `canonicalize_migrations.py` (token-aware parse, cross-boundary deps,
  atomic install), `check_migrations.py` (token-aware draft parse, released-dep
  resolution), `ops/release.rs` (canonicalize stage), `MIGRATIONS.md` +
  `drafts/README.md` rewrite, Python tests, Rust store + release tests, two-generation
  fixtures.
- Out of scope: changing the canonical migration file format or the runner; any
  change to `lf install` promotion (only preserve it); a generic multi-product
  release platform.

## Done when

- `lf release run` canonicalizes accumulated drafts inside its worktree after the
  version bump and before the commit; the release PR carries the generated canonical
  migrations and registry entries (Rust integration test).
- `new_migration.py` mints an immutable draft id, writes a file-only draft, edits no
  Rust registry, and validates `--depends-on` against drafts ∪ released names.
- Two `new_migration.py` runs of the same name produce two distinct draft files and
  leave `migrations.rs` byte-identical (Python test).
- A draft depending on an already-released migration canonicalizes and orders after
  it; a draft depending on an unknown name fails (Python test).
- A forced graph/write/registry failure leaves the drafts dir, canonical dir, and
  `migrations.rs` byte-identical (Python test).
- A real database initialized at a prior generation upgrades cleanly through the
  current canonical tail, and a fresh database initializes to the same schema (Rust
  store tests over committed two-generation fixtures under `tests/`).
- W2-319 promotion safety is unchanged (`lf install` promote untouched; ordinals
  monotonic).
- `cargo fmt`/`clippy`/`test`, `check_migrations.py`, and the Python suite pass in CI
  on the Task PR before it is submitted for Project review.

## Measure

- Merge-conflict rate on `migrations.rs` for concurrent migration-adding branches:
  target 0 (was: every concurrent pair contends on ordinal or the DRAFTS slice).
- Canonicalization failure leaves 0 files changed (`git status --porcelain` empty
  after a forced-failure run).
