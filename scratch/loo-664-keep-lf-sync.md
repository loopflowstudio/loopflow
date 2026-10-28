# LOO-664: Keep sync working with ignored telemetry

2026-10-27. Accepted outcome; selective checkpoint implemented; gate and delivery remain.
Jack Heart authorized the bounded hotfix and autonomous shipping in comment
`297df81a-24b3-4431-8336-a0dfeb3660ab`. Iteration feedback rejects the previous
pre-staged telemetry exception. The implementation mechanism follows source and
fixture evidence; it was not separately reviewed.

## Outcome and demo

Installed 0.14.19 stopped LOO-661 after staging ordinary edits because Apple Git
2.50.1 rejected the ignored telemetry exclusion. The repair serves Infrastructure's
KR that a Session can advance a Task without interruption by Loopflow plumbing;
no numeric target was supplied.

Automatic feature-branch sync must checkpoint ordinary staged and working edits,
merge upstream, and preserve local `.lf/tmp/` telemetry's staged/working partition.
Missing or telemetry-only edits create no checkpoint; repeat sync leaves HEAD
unchanged. Ordinary partial staging is consolidated. Previously committed telemetry
stays in history, but its local changes must not enter new checkpoints.

Demo through `lf sync origin/main` in disposable feature checkouts with local bare
remotes: combine upstream changes, ordinary edits and distinct staged/working
telemetry; repeat with/without an ignore rule and sync twice. Verify checkpoint
contents, upstream ancestry, unchanged telemetry diffs and no second checkpoint.
No credentials or display are required.
The existing ops metric writer may append a new sync observation; preservation
means retaining prior telemetry bytes and staging, not suppressing that writer.

## Implementation and deletion boundary

`run_sync_in` in `rust/loopflow/src/lf/commands/ops/mod.rs` checkpoints before
`MergeTarget`, then replans. Graph sync uses the same CLI path. Main, manual,
plan-only, continue and abort retain their behavior. Git owns refs, indexes,
working contents and recovery objects.

`checkpoint_uncommitted_work`:

1. Stages ordinary edits with the rooted exclusion `:(exclude,top,glob)[.]lf/tmp/**`.
   A literal `.lf/tmp` prefix fails when ignored, even with a recursive glob suffix.
2. Loads the staged tree into a temporary index and restores telemetry from HEAD
   there. This retains explicitly staged ignored user files while leaving caller
   telemetry staging intact. Starting at HEAD and adding working files would
   lose the explicitly staged ignored files.
3. Commits only if that index differs from HEAD, through the existing Git commit
   owner, then refreshes ordinary real-index entries using the same exclusion.
   This follows the selected-document commit pattern without a new shared API.
4. Leaves residual telemetry to `with_preserved_edits`, whose `stash apply --index`
   restores the partition and whose failure retains the named recovery stash.
   Ignored untracked telemetry stays on disk; tracked and non-ignored edits use
   the existing preservation path.

Checkpoint errors prevent merging and preserve real edits. A refresh failure after
commit reports that the checkpoint was saved, without implying rollback or success.

Upstream changes to tracked telemetry can conflict during restoration. Existing
recovery retains the stash and reports its exact restore command; sync must fail
without deleting that recovery entry.

### Delete — do not maintain

- `stage_uncommitted_work` and its whole-index commit are removed; no fallback.
- The literal-directory exclusion and pre-staged telemetry exception are removed.
  Exclusion alone cannot remove already-staged entries; resetting the real index
  would destroy their partition. Plain `git add -A`, force-add, swallowed failures
  and filename enumeration remain rejected.
- No remaining deletion targets. Keep resolver, scratch preservation, strategy
  and existing CLI tests. Preserve LOO-312 / PR #1656's independent onboarding
  work in the same module. No schema, DTO, Desktop or Task-state changes.

Original exploration: `e94c547222:scratch/loo-664-keep-lf-sync.md`.
Pre-compression design and evidence: `3796c6c8f9:scratch/loo-664-keep-lf-sync.md`.

## Acceptance and remaining work

**This slice:** the selective checkpoint and six `sync_preserves_work_with_`
regressions remain one change. They cover ignored/unignored telemetry; additions,
modifications, deletions and renames; unusual names and neighboring/nested ordinary directories;
explicitly staged ignored user files; existing telemetry history; repeat sync;
index-lock failure retaining HEAD/index/work; and upstream telemetry conflicts
retaining staged/working bytes in the named recovery stash without reporting
success. Binary diffs check telemetry's full staged/working partition.

Gate, publication and landing remain. Gate includes existing resolver/Graph-sync
tests and commit-boundary suites:

```bash
uv run python scripts/test.py --reuse-passing --rust-filter 'test(sync_tests::) | test(commit_tests::) | test(agent_startup_tests::) | test(release_tests::) | test(release_lock_tests::)' < /dev/null
```

The affected gate must exit zero, including formatting and Clippy; CI owns
cross-platform coverage. `commit_tests::` covers the reused separate-index pattern
and Git failure diagnostics; the test runner isolates inherited `LF_*` variables.
Caller Graph owns publication and landing. Infrastructure
owns the single coordinated release/install and installed LOO-661 readback; this
Task supplies its merged revision, with no competing publisher or installation.
Source fixtures establish no installed recovery.

Done when the actual CLI scenarios above pass, genuine Git failures retain work
and actionable recovery, and the repair lands with an exact merged revision for
Infrastructure's release. Installed LOO-661 recovery remains a separate readback
after that release; a passing source fixture cannot close that obligation.

Until the fixed binary is installed, checkpoint intended edits with explicit
`lf commit` paths before returning to Graph sync, including this Task's authorized
Wave-memory contribution. Preserve other work; leave release/install to Infrastructure.

Check: `git diff --check` and `lf context` passed on 2026-10-27 (both budgets within limits); prior unchanged-source evidence: `scripts/test_network.py cargo +stable test -p loopflow --test integration sync_preserves_work_with_ < /dev/null` — 6 passed, `cargo +stable fmt --all -- --check` and network-isolated `cargo +stable clippy --all-targets -- -D warnings` passed; affected suites remain with gate.
