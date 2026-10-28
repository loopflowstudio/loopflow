# LOO-664: Keep sync working with ignored telemetry

2026-10-27, reconciled after compression. Selective checkpoint implemented;
gate and delivery remain. The approach is accepted under the Task's hotfix scope.
The current Task definition requires no additional implementation slice.
Jack Heart authorized the bounded hotfix and autonomous shipping in comment
`297df81a-24b3-4431-8336-a0dfeb3660ab`. Iteration feedback rejects the previous
pre-staged telemetry exception. The mechanism follows source and fixture evidence,
without separate design review.

## Outcome and demo

Installed 0.14.19 stopped LOO-661 after staging ordinary edits because Apple Git
2.50.1 rejected the ignored telemetry exclusion. Infrastructure's relevant KR is
advancing Tasks without interruption by Loopflow plumbing; no numeric target.

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

Checkpoint errors prevent merging and preserve edits. A refresh failure after
commit reports that the checkpoint was saved. Upstream telemetry conflicts fail
with the retained stash and its exact restore command.

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

Seven `sync_preserves_work_with_` regressions cover ignored/unignored telemetry;
additions, modifications, deletions and renames; unusual names and neighboring/nested directories;
explicitly staged ignored user files; existing telemetry history; repeat sync;
index-lock failure retaining HEAD/index/work; and upstream telemetry conflicts
retaining staged/working bytes in the named recovery stash. Binary diffs check
telemetry's full staged/working partition. A whitespace-only filename regression
guards against trimming `diff --name-only` into a false empty checkpoint.

This slice: gate, publication and landing. Gate includes existing resolver/Graph-sync
tests and commit-boundary suites:

```bash
uv run python scripts/test.py --reuse-passing --rust-filter 'test(sync_tests::) | test(commit_tests::) | test(agent_startup_tests::) | binary(release_tests) | test(release_lock_tests::)' < /dev/null
```

Gate must pass, including formatting and Clippy; CI owns cross-platform coverage.
`commit_tests::` covers the reused separate-index pattern and Git diagnostics;
the runner isolates inherited `LF_*`. Caller Graph owns publication and landing,
then supplies Infrastructure the exact merged revision. Infrastructure alone owns
coordinated release/install and installed LOO-661 readback. Source fixtures and
landing cannot close that installed verification obligation.

The LOO-664 Wave-memory contribution is committed in
`0f6969856dcda27735c9e67520116f9e7684616d`; preserve it and other memory.
Installed 0.14.19 still fails on clean protected branches with ignored telemetry.
Infrastructure reports a pinned candidate passing that reproduction and a signed
CLI/matching Desktop helper passing exact-frontier installation preflight.
Installation and LOO-661 readback remain pending; explicit-path checkpoints alone
cannot restore sync, and source/candidate passes do not prove installed acceptance.

The 2026-10-28 03:39 UTC Task update authorizes a temporary Task-local bridge:
after validating and checkpointing each step, atomically move `.lf/tmp` into a
unique durable archive, leaving it absent immediately before automatic sync.
The compress archive is present at
`/Users/jack/benchmarks/installed-sync-hotfix-20261027/task-telemetry/LOO-664-compress-HbQpjT3c/telemetry`
and remains separate from fresh sync metrics. This realign step reserves
`/Users/jack/benchmarks/installed-sync-hotfix-20261027/task-telemetry/LOO-664-realign-91MrUmnG/telemetry`
for those fresh bytes after checkpointing. This avoids the installed literal-directory
error; it is not the shipped repair or permission to change other checkouts or ignore rules.

Reconciliation found no implementation mismatch or new product decision. The sync
implementation and regressions are unchanged since `d2110131e0`; integration of
upstream `0d72f072a4` changes neither. Infrastructure and its release child's goal
and memory agree on operation-entry proof and separate installed acceptance.
The existing Wave lesson now also retains the whitespace-filename counterexample.

Check (2026-10-27): `git diff --check` and `lf context` passed; prior network-isolated `cargo +stable test -p loopflow --test integration sync_preserves_work_with_` (7 passed), formatting and Clippy results remain applicable to unchanged code; affected suites remain with gate.

## Concept review — 2026-10-27

No design change and no open product choice. Jack Heart's accepted scope already
settles the consequential distinction: local telemetry is excluded from new
checkpoints even when staged; ordinary edits retain consolidated checkpointing.
The implementation and `docs/ops.md` express that contract without a new flag,
ignore-rule requirement, public type, persistence record or skill instruction.

| Action | Concept | Existing owner after this repair |
| --- | --- | --- |
| Run `lf sync origin/main` on a feature branch | Ordinary work becomes a checkpoint; telemetry remains local | `run_sync_in` → `checkpoint_uncommitted_work` → Git commit |
| Merge upstream with residual local telemetry | Preserve staged and working contents separately | `with_preserved_edits` → Git index/worktree and stash |
| Recover from a telemetry restoration conflict | Retained edits have a named recovery object | Existing error gives `git stash apply --index <sha>` after resolving the Git operation |

The former mismatch was treating the entire staged index as checkpoint content
after excluding telemetry only from staging. The temporary index resolves that
mismatch while keeping Git as the sole owner of content and recovery. No further
lifecycle or abstraction needs to be introduced or removed.

Normal sync checkpoints work, incorporates upstream and restores telemetry; a
repeat creates no checkpoint. A staging failure stops before merging and remains
an error. A post-commit refresh failure names the saved checkpoint; restoration
conflicts retain both telemetry versions in the recovery stash. Recovery must not
be described as automatic success when upstream changed the same telemetry.

Keep the counterexamples already represented in the design and fixtures:
pre-staged telemetry defeats exclusion alone; explicitly staged ignored work
defeats rebuilding the checkpoint from HEAD plus ordinary additions; a whitespace
filename defeats trimmed-output emptiness detection. These constrain the repair,
not new product choices. Next proof remains the affected gate and Infrastructure's
separate installed readback; this review changes no code and reruns no Rust checks.

## Publication verification — 2026-10-27

Check: `scripts/test.py --reuse-passing --rust-filter 'test(sync_tests::) | test(commit_tests::) | test(agent_startup_tests::) | test(release_tests::) | test(release_lock_tests::)'` passed architecture, formatting, Clippy, 77 Rust tests and 72 website tests; the standalone release binary required the corrected selector above.
Check: network-isolated `scripts/materialize_rust_tests.py --prepare-cli -- cargo nextest run -p loopflow --test release_tests --no-tests fail --no-fail-fast --build-jobs 4 --test-threads 4` passed all 76 tests with rustup stable and inherited LF variables removed.

No source repair was needed. The publication boundary has local proof; hosted CI,
landing, coordinated release/install and installed LOO-661 readback remain separate.
The Task-local bridge retains pre-publication telemetry (including gate logs) at
`/Users/jack/benchmarks/installed-sync-hotfix-20261027/task-telemetry/LOO-664-pr-publish-acgse2al/before-sync` and reserves `/Users/jack/benchmarks/installed-sync-hotfix-20261027/task-telemetry/LOO-664-pr-publish-acgse2al/after-publication`
for fresh telemetry before returning to automatic Graph sync. Both are atomic moves;
prior archives remain untouched.

Check: after conflict-free sync with `fd2b2c18b5`, network-isolated `cargo test -p loopflow --test integration documented_commands:: --jobs 4` passed all 4 checks; the sync implementation and regressions were unchanged. PR #1754 is published at https://github.com/loopflowstudio/loopflow/pull/1754; merge and installed acceptance remain with their existing owners.
