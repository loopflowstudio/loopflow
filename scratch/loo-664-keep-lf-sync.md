# LOO-664: Keep sync working with ignored telemetry

2026-10-27. Status: pre-staged telemetry repair implemented; focused CLI tests pass.
Jack Heart authorized the bounded hotfix and autonomous shipping in comment
`297df81a-24b3-4431-8336-a0dfeb3660ab`. Iteration feedback rejects the previous
exception allowing pre-staged telemetry into checkpoints. Gate and delivery remain.
The outcome and shipping scope are accepted; the implementation mechanism is
selected from source inspection and fixture evidence, not separately reviewed.

## Problem and demo

Installed 0.14.19 stopped LOO-661 after staging ordinary edits because Git rejected
the ignored telemetry exclusion. Autonomous Tasks need sync to finish without a
manual Git workaround. This serves Infrastructure's KR that a Session can advance
a Task without interruption by Loopflow plumbing; no numeric target was supplied.

Demo: in a disposable feature checkout with upstream changes, ordinary edits,
and distinct staged/working telemetry, run `lf sync origin/main`. Ordinary edits
appear in the checkpoint, upstream appears in HEAD, and telemetry retains both
local versions without entering the checkpoint. Repeat with and without the
ignore rule, then sync again: HEAD stays unchanged on the second sync. Local
bare remotes suffice; no credentials or display are required.

## Outcome and architecture

Automatic feature-branch sync checkpoints ordinary staged and working edits,
merges upstream, and preserves local `.lf/tmp/` telemetry, including distinct
pre-staged and working contents. Missing or telemetry-only edits create no
checkpoint. Ordinary partial staging is consolidated into the checkpoint.
Telemetry already committed in history is not removed or rewritten; its local
changes must not enter new checkpoints.

Git remains authoritative for the index, working tree, refs and recovery objects.
`run_sync_in` in `rust/loopflow/src/lf/commands/ops/mod.rs` owns the checkpoint
before a `MergeTarget`, followed by replanning. Graph sync uses the same path.
Main, manual, plan-only, continue and abort retain their current behavior.

The whole-index checkpoint is replaced by `checkpoint_uncommitted_work`, which:

1. Stages ordinary edits using `git add -A -- :/ :(exclude,top,glob)[.]lf/tmp/**`.
2. Loads that staged tree into a temporary index and restores only `.lf/tmp/`
   from HEAD there. The real index retains caller-staged telemetry. Starting
   from the staged tree also retains explicitly staged, otherwise ignored user files.
3. Commits only if that temporary index differs from HEAD, through the existing
   Git commit owner, then refreshes ordinary real-index entries against HEAD.
4. Leaves the existing `with_preserved_edits` merge wrapper to retain residual
   telemetry and restore its staged/working partition with `stash apply --index`.
   Recovery failure retains the named stash instead of dropping caller data.

The temporary-index pattern matches selected-document commits. A checkpoint
error propagates without merging; real edits remain staged/on disk. If the commit
succeeds but real-index refresh fails, the error says the checkpoint was saved;
it must not imply rollback or completed sync. No new network call, provider launch,
persisted model or recovery owner is introduced.

## Delete — do not maintain

- Removed `stage_uncommitted_work` and its whole-index commit call in favor of the
  selective checkpoint helper; no alternate path or fallback remains.
- Removed the docs and assumptions accepting pre-staged telemetry in checkpoints.
- The earlier literal-directory exclusion was already deleted at `e94c547222`.
  Keep the rooted glob: Apple Git 2.50.1 rejects a literal ignored `.lf/tmp`
  prefix after staging ordinary edits, even with a trailing recursive glob.

Keep resolver, scratch preservation, strategy, and existing CLI regression tests.
LOO-312 / PR #1656 changes onboarding in the same module, outside this helper;
preserve those independent changes. No schema, DTO, Desktop or Task-state changes.

## Evidence and constraints

The rooted exclusion alone fixes the ignored-directory error but cannot remove
telemetry already in the index. Committing that index violates the accepted
outcome. Resetting telemetry in the real index would lose its staged/working
partition; the temporary index avoids that mutation. Starting it at HEAD and
staging only ordinary working paths would miss explicitly staged ignored user
files, so the selected implementation starts from the caller's staged tree.
Plain `git add -A`, force-add, and ignoring Git failures remain rejected: they
respectively admit non-ignored telemetry, admit unrelated ignored work, or hide
real failures. Filename enumeration adds parsing without resolving pre-staging.

The earlier four CLI tests cover ignored/unignored telemetry, additions, edits,
deletions, renames, unusual names, root-neighbor and nested user paths, checkpoint
contents, upstream ancestry, retry, missing/telemetry-only state, and an index
lock preserving HEAD/index/work. Original mechanism exploration and rejected
alternatives: `e94c547222:scratch/loo-664-keep-lf-sync.md`.

The pre-staged regression covers telemetry additions, modifications and deletions,
distinct staged/working contents, and an unchanged committed telemetry tree. It
runs ignored/unignored cases with/without ordinary work, including explicitly
staged ignored user files and repeat sync. Shared CLI assertions report both Git
output streams; binary diff comparisons verify the full staged/working partition.

Forbidden outcomes: committing local telemetry changes; force-adding ignored
files; discarding staged/working contents; swallowing Git errors; requiring ignore
configuration; excluding similarly named ordinary directories; claiming source
proof establishes installed LOO-661 recovery.

## Remaining work and acceptance

- **This slice:** retain the implemented selective checkpoint and its five
  `sync_preserves_work_with_` regressions as one change. No implementation split,
  fallback or further product decision is needed before gate.
- Gate owns affected suites, including existing resolver and Graph-sync
  preservation checks. Formatting and Clippy passed before the explicit-path
  checkpoint requested by Jack Heart after installed sync stopped the prior
  Graph. Commit-boundary suites required by the test guide are included below.
- Caller Graph owns publication and landing. Infrastructure owns the single
  coordinated release/install and installed-sync readback; this Task supplies
  its merged revision. No competing publisher or candidate installation.
- Proposed memory correction for the existing LOO-664 Gotcha: pre-staged telemetry
  is preserved outside new checkpoints. Wave memory is read-only in this step;
  the existing uncommitted memory contribution remains untouched.

```bash
TMPDIR=/private/tmp uv run python scripts/test.py --reuse-passing --rust-filter 'test(sync_tests::) | test(agent_startup_tests::) | test(release_tests::) | test(release_lock_tests::)' < /dev/null
```

Done when the actual CLI fixtures prove ordinary edits in checkpoint and merged
HEAD, upstream ancestry, unchanged local telemetry index/working diffs, no extra
checkpoint for telemetry-only state or repeat sync, and actionable index-lock
failure retaining HEAD/index/work. Existing Graph sync and resolver behavior must
also pass. The affected gate must exit zero, including formatting and Clippy;
CI owns cross-platform coverage. Merged source is the delivery boundary here;
installed recovery remains unproved until Infrastructure's released-binary readback.

Check: `TMPDIR=/private/tmp uv run python scripts/test_network.py cargo test -p loopflow --test integration sync_preserves_work_with_ < /dev/null` — 5 passed (unchanged source, result reused); `cargo +stable fmt --all -- --check`, `cargo +stable clippy --all-targets -- -D warnings`, and `git diff --check` passed; affected suites remain with gate.
