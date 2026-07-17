# Assign canonical migration order at the release cut

## Decision

A migration has two different coordinates:

- a **source id**, minted once when the schema change is authored and preserved
  forever; and
- a **canonical ordinal**, assigned once by the release PR.

Dependencies use the source id. Databases record the canonical file stem. A
human-readable name explains the change but identifies nothing.

This iteration replaces the shared, name-keyed draft registry with a file-only
source model and makes release preparation an error-atomic transaction. The two
changes belong together: independent branches do not share a registry edit, and
the release cut is the first operation that writes a shared order.

## Problem

Today `scripts/new_migration.py` allocates an ordinal on a feature branch. Two
parallel branches can therefore choose the same ordinal and must coordinate on a
rebase even when their schemas are independent. `check_migrations.py` compounds
that cost by requiring every canonical file on `origin/main` to exist locally;
a branch can fail only because main advanced after its branch point.

The first draft design moved the race instead of removing it:

- `<human_name>.sql` still made every branch coordinate on a global name;
- every author still edited the same `DRAFTS` slice;
- `depends_on` accepted only live drafts, so a dependency broke when its upstream
  draft became canonical;
- canonicalization wrote files and the registry incrementally;
- one `previous_release.db` was both the release gate's input and the next
  release's output; and
- the unchanged `origin/main` missing-file check made the claimed zero false reds
  impossible.

The current implementation also makes Rust CI red:
`every_migration_file_is_registered_under_its_own_name` treats the new `drafts/`
directory as an unregistered migration. Generated registration replaces that
test's hand-maintained premise rather than merely teaching it to ignore one
directory.

## Authoring contract

```bash
uv run python scripts/new_migration.py add_wave_colour
# rust/loopflow/src/store/migrations/drafts/
#   task-a3d1cb8f8cf24f2bb0d47c15fcf75fd2--content-7f3a91c2a6d14b98__add_wave_colour.sql
```

The file is the entire branch-owned change:

```sql
-- depends_on: task-4d94...--content-29af...
ALTER TABLE waves ADD COLUMN colour TEXT;
```

The stem is `<source-id>__<readable-name>`.

- `source-id` is opaque and immutable. `new_migration.py` uses the stable Task
  UUID when Loopflow supplies one and always adds a generated content UUID, so a
  Task may own more than one migration. Outside a Task it emits a content UUID
  alone. The chosen id is persisted in the filename; release retries never
  regenerate it.
- `readable-name` is snake_case prose. It is not globally unique, is never a
  dependency target, and is never used as a topological tie-break.
- `depends_on` contains source ids, not names or ordinals. It may name a draft or
  an already-canonical migration.
- The SQL file may be edited while its PR is active, but its source id does not
  change. Once canonical, the full file name and bytes are immutable.

`new_migration.py` creates only this file. It does not fetch, count files, edit
Rust, or inspect `origin/main`. Two branches therefore have disjoint diffs even
when their readable names match.

## Generated registration

`rust/loopflow/build.rs` scans both migration directories and writes
`$OUT_DIR/migration_registry.rs`. `migrations.rs` includes that generated file;
there is no `DRAFTS` or `MIGRATIONS` slice for an author or the release script to
edit.

The generator emits:

```rust
pub struct DraftMigration {
    pub source_id: &'static str,
    pub name: &'static str,
    pub depends_on: &'static [&'static str],
    pub sql: &'static str,
}

const MIGRATIONS: &[Migration] = &[/* canonical files, numeric id order */];
const DRAFTS: &[DraftMigration] = &[/* draft files, source-id order */];
```

Build generation validates file grammar, duplicate source ids, duplicate
canonical ordinals, and exact file/registry coverage. It emits
`cargo:rerun-if-changed` for both directories. `check_migrations.py` performs the
same user-facing validation without requiring a Rust compile; Rust tests prove
that the generated sets equal the files on disk.

New canonical files retain the source id:

```
0.11.030__task-a3d1...--content-7f3a...__add_wave_colour.sql
```

Canonicalization copies the draft bytes, including dependency comments, and
adds only the release ordinal to the path. Existing released files are not
renamed. The generator assigns each legacy file a stable synthetic source id
equal to `legacy:<existing-file-stem>`, so a new draft can explicitly depend on
one without changing history.

`Migration` gains explicit `version`, `source_id`, and `name` fields generated
from the file. `version` remains exactly the canonical file stem recorded in
`schema_migrations`; formatting it is no longer reconstructed from a supposedly
unique human name.

## Stable dependency resolution

Validation builds one `source_id -> canonical | draft` map.

1. Duplicate ids anywhere are ambiguous and fail.
2. A dependency resolved to canonical is already satisfied.
3. A dependency resolved to a draft contributes a graph edge.
4. A dependency missing from both sets fails.
5. Kahn ordering chooses the lowest **source id** among ready nodes. Readable
   name, PR number, merge time, file mtime, and canonical ordinal do not decide
   draft order.

This survives a release boundary. If draft B depends on source id A and A is
canonicalized by release N, B still names A after rebasing and release N+1
resolves it from the canonical set. No header or dependency rewrite is needed.

Development stores apply canonical migrations, then the resolved draft order,
recording `draft.<source-id>` plus a checksum. A missing, renamed, or changed
draft receipt recreates only that source-owned development database. Released
builds are produced from a canonicalized tree with no drafts, so no published
binary can write a `draft.*` receipt. The existing W2-319 authority and shared
store fence remain unchanged.

Data-transform tests use `apply_through_source_id`, which resolves the same id
from either registry. The test therefore survives draft-to-canonical promotion.

## Branch migration check

`origin/main` is no longer required to be a subset of the branch. That rule is
the behind-main false red.

`check_migrations.py` instead separates two questions:

- **Published immutability:** every canonical file in the last release tag still
  has the same path and bytes.
- **Branch-authored change:** compare the branch with `merge-base(HEAD,
  origin/main)`. A modification or deletion of a canonical file fails. A new
  canonical file is legal only when the same branch deletes a draft with the
  same source id and bytes and the target namespace matches the bumped package
  version. That is the release transformation.

Canonical files added to `origin/main` after the branch point are not in the
branch-authored diff, so their absence is ignored. A real collision, mutation,
missing dependency, or malformed graph remains red. CI still fetches
`origin/main` to find the merge base, but never asks a behind branch to contain
main's whole tree.

## Release transaction

`lf release run` keeps its existing worktree, PR, merge, and exact-commit tag.
Preparation changes order:

1. Resolve the previous tag and target version, create a fresh release worktree,
   then **bump manifests first**. The bumped version selects the namespace.
2. Freeze a manifest of every draft source id, path, dependency, and byte digest,
   plus the exact previous fixture digest.
3. Build a pure `CanonicalizationPlan` in memory. Validate all ids,
   dependencies, cycles, target paths, ordinals, and fixture generations before
   touching the worktree.
4. Stage the canonical file tree in a temporary sibling directory. Record a
   byte-for-byte before image for every migration and fixture path the operation
   can add, replace, or remove; keep that transaction open through verification.
5. Install the canonical paths with temp-file writes and `os.replace`. Any
   returned write error restores the before image and removes every staged
   addition.
6. With canonical files now present, build and run the migration verifier **from
   that modified worktree before the release commit**. The already-running
   release binary does not contain files it just canonicalized, so it must not
   perform this proof itself. The worktree-built verifier copies the prior
   fixture to a temporary target, migrates it, and proves checksums, schema,
   `PRAGMA foreign_key_check`, fresh initialization, and a no-op second open.
   Install the verified target fixture, prune only a generation older than the
   prior one, then run `check_migrations.py` and the generated-registry test. Any
   build, fixture write, or verification error restores the same before image.
   From the caller's perspective the operation is atomic: success installs the
   whole plan; failure leaves the migration and fixture trees byte-identical. A
   process crash can strand only the disposable release worktree, which is
   discarded before a retry.
7. Commit and publish the release PR. Wait for the exact PR head's required
   `rust-test`, `rust-lint`, `migration-check`, and `python-test` checks to pass
   before arming merge. Only after the PR merges is that merged SHA tagged.

Canonicalization no longer rewrites Rust source. Its installed diff consists of
draft deletions, canonical file additions, the target fixture generation, and
normal release metadata.

## Two-generation fixtures

Fixtures are versioned, never overwritten in place:

```
rust/loopflow/tests/fixtures/migrations/v0.11.3.db  # prior input
rust/loopflow/tests/fixtures/migrations/v0.11.4.db  # target output
```

Release `v0.11.4` copies `v0.11.3.db` to a temporary database, migrates that
copy through the canonical tail, validates it, and installs the result as
`v0.11.4.db`. The prior file remains present and byte-identical in the release
PR. Once the target is valid, a fixture older than the prior generation may be
removed in the same transaction, leaving exactly prior + target.

Rust tests open both committed generations against the current chain. In a
release PR this proves the prior input upgrades and the target is a no-op; after
release it continues to exercise one real historical upgrade. A separately
initialized temporary database proves fresh schema equivalence. No test decides
that the newly generated target fixture was the previous release.

Fixture materialization is deterministic. After the real migration engine and
data checks pass, the worktree-built verifier normalizes non-semantic ledger
fields (`applied_at` and build/source provenance) to documented fixture values,
checkpoints/removes WAL state, and uses `VACUUM INTO` for the committed file. A
retry test proves the resulting target fixture digest is stable; production
databases still retain their real timestamps and provenance.

## Proof matrix

| Proof | Executable evidence |
| --- | --- |
| Two independent branches share no registry edit | A temporary git repo forks branches A/B; each `new_migration.py` run changes exactly one distinct draft file and neither changes `migrations.rs` or `build.rs`. Both merge orders compile to the same generated registry. |
| A dependency crosses a release boundary | Canonicalize source A into release N, leave B depending on A's source id, then prepare release N+1. B resolves A from canonical files and orders after it. |
| Invalid graph leaves the tree unchanged | Hash every relative path and byte under migrations/fixtures, run missing-dependency and cycle plans, and assert the complete digest is unchanged. |
| Write failure leaves the tree unchanged | Patch the file side effect in the canonicalizer to fail after the first successful replacement; rollback must restore the same complete-tree digest and leave no temp path. |
| Release PR retains its prior input | Prepare a release from `vN.db`; assert `vN.db` still exists with its original digest and the diff adds `vN+1.db` rather than modifying `vN.db`. |
| Retry is deterministic | Run preparation twice from the same frozen tree and target version; canonical paths, bytes, fixture bytes, and the resulting diff are identical. |
| Current Rust red is closed | The generated registry/file equality test passes and no directory entry can be interpreted as a migration. Full required Rust CI passes on the release PR head. |

## Files to change

- `scripts/new_migration.py`: mint one immutable source id and write one file.
- `scripts/check_migrations.py`: parse source ids/dependencies; replace the
  origin/main superset rule with merge-base branch-authorship checks.
- `scripts/canonicalize_migrations.py`: pure plan, cross-boundary resolver,
  staged transactional install, fixture rotation.
- `rust/loopflow/build.rs`: generate canonical and draft registration.
- `rust/loopflow/src/store/migrations.rs`: include generated sets, apply drafts
  by source id, recreate stale dev stores, expose source-id test helpers.
- `rust/loopflow/src/ops/release.rs`: bump -> canonicalize -> verify -> commit ->
  wait for required CI -> merge -> tag.
- A worktree-built internal migration verifier: exercise the just-generated
  registry and materialize the normalized target fixture; the parent release
  process only orchestrates it.
- `rust/loopflow/tests/fixtures/migrations/`: retain versioned prior and target
  databases.
- `python/tests/test_{new,check,canonicalize}_migrations.py` and Rust migration/
  release tests: the proof matrix above.
- `rust/loopflow/src/store/MIGRATIONS.md`, CI comments, and doctor output: one
  publication boundary and no hand-edited registry instructions.

The current partial Python implementation is revised in place. Do not merge the
new draft format before generated registration, cross-boundary resolution,
transactional release preparation, and the fixture/CI gates exist end to end.

## Done when

- `new_migration.py` changes exactly one uniquely keyed draft file and performs
  no git/network or shared registry write.
- Two same-name drafts from independent branches coexist; generated registration
  includes both.
- Dependencies resolve by source id from drafts or canonicals; a release-boundary
  dependency test passes.
- Missing/ambiguous/cyclic graphs and injected write failure leave the complete
  migration + fixture tree byte-identical.
- Canonicalization runs after the version bump, assigns one deterministic
  contiguous tail, consumes included drafts, and is retry-identical.
- The release PR contains byte-identical prior input and a separately named
  target fixture; Rust proves prior upgrade, target no-op, and fresh init.
- A branch behind main passes when its only difference is canonical files added
  upstream after its merge base. Real branch-authored canonical changes still
  fail.
- Worktree migration verification passes before commit; required Rust CI passes
  on the exact PR head before merge; the merged SHA is the tagged SHA.
- Development draft drift recreates only the per-source dev store. W2-319's
  shared-store promotion fence is unchanged.

## Measure

The previous document claimed all behind-main reds would become zero without
removing the check that caused them. The measurable claim is narrower:

- **Shared registration edits per schema PR:** target 0. A schema PR adds its
  uniquely keyed file; generated code owns registration.
- **Behind-main absence-only failures:** target 0. Count failures whose only
  difference is canonical files added to `origin/main` after the branch's merge
  base. Invalid graphs and branch-authored canonical mutations are expected reds.
- **Pre-release ordinal renames:** target 0. Feature PRs contain no ordinal.
- **Release retry drift:** target 0 changed bytes for identical frozen input and
  target version.

Historical CI does not record a structured cause for every migration red, so no
numeric baseline is invented. The live 027/028/029 sequence and this PR's Rust
registration failure are concrete pre-change evidence; post-change tests and
categorized check failures make the four targets directly computable.
