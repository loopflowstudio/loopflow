# LOO-664: Keep sync working with ignored telemetry

2026-10-27. Status: implemented, focused CLI regressions passing; bounded hotfix and autonomous
shipping authorized by Jack Heart in comment
`297df81a-24b3-4431-8336-a0dfeb3660ab`. Mechanism selected during kickoff,
not separately reviewed. Gate and delivery remain outstanding.

## Problem and demo

Installed 0.14.19 stopped LOO-661 at sync. Its staging command updates the index,
then exits 1 because the exclusion path names an ignored directory. Autonomous
work needs to checkpoint edits and reach the merge without a manual Git repair.
Local telemetry must stay local even without an ignore rule.

Demo: in a disposable feature checkout with upstream changes, staged work,
unstaged edits, and `.lf/tmp/metrics/ops.jsonl`, run `lf sync origin/main`.
It checkpoints the real edits, incorporates upstream, and finishes successfully.
The checkpoint contains the intended additions, edits, deletions and renames;
telemetry remains on disk and is absent from the commit. Repeat without the
telemetry ignore rule, with the same result. No credentials or real remote needed.

This serves Infrastructure's KR that a Session can advance a Task without
Loopflow plumbing interrupting it. No quantitative performance target was supplied.

## Current and target architecture

Git's index, working tree, commits and refs are authoritative. CLI
`run_sync_in` in `rust/loopflow/src/lf/commands/ops/mod.rs` selects a plan and,
for an automatic feature-branch `MergeTarget` without intervention, invokes
`stage_uncommitted_work`, checkpoints using `git::commit`, then replans.
The merge and resolver use the existing recovery machinery. Graph `op: sync`
enters the same command implementation. Main, manual, plan-only, continue and
abort paths do not use this staging helper.

Keep that ownership and sequence. Replace the exclusion argument with:

```text
git add -A -- :/ :(exclude,top,glob)[.]lf/tmp/**
```

Arguments are passed directly to Git, without shell expansion. `top` anchors
the exclusion to the checkout root; `glob` and trailing `/**` cover descendants,
including hidden files. `[.]` matches exactly the dot while avoiding a literal
ignored directory prefix. Keep a short explanatory comment: simplifying this
back to `.lf/tmp/**` restores the observed failure.

Retain `git_stdout` error propagation and the existing staged-diff check.
No filename parsing, index reset, force-add, ignore-rule mutation, extra Git
subprocess, or fallback algorithm is needed. Successful sync still consolidates
staged and unstaged real edits into its checkpoint; preserving a partial-staging
partition is not a new promise of this repair.

## Risk findings and alternatives

- Apple Git 2.50.1 reproduced the original failure with inherited `GIT_*` removed
  and global/system configuration disabled. Real edits were staged despite exit 1.
  Ignoring that exit would conceal genuine failures and is rejected.
- Adding `/`, `/**`, or `glob` to the original literal directory prefix still
  fails when `.lf/tmp/` is ignored. A wildcard only in the final directory name
  without a descendant pattern failed to exclude nested telemetry in the
  non-ignored case. These are not viable repairs.
- The selected glob passed exact staged-path and checkpoint-content checks with
  no ignore, `.lf/tmp/`, root `/.lf/`, and `.lf/tmp/*`, each with telemetry
  present and absent. Nested/hidden telemetry stayed on disk and out of commits.
  Root-neighbor `.lf/tmp-neighbor/` and nested `nested/.lf/tmp/` remained ordinary
  user paths whenever not ignored by Git. Spaces, Unicode, newlines, leading
  dashes and pathspec-looking filenames staged correctly.
- An existing index lock still fails with an actionable Git error, retaining
  the original index bytes and working edits. No success can be inferred from
  partial staging after an error.
- Plain `git add -A` avoids this failure but admits non-ignored telemetry.
  `-f` would admit unrelated ignored files. Both violate the brief.
- Enumerating eligible paths with `ls-files -z` and feeding literal NUL-delimited
  paths to `git add` could work, but adds parsing, another subprocess and a
  read/stage interval. The direct glob resolves the demonstrated problem without
  introducing another staging implementation.

Git documents ignored exact paths as errors and describes rooted exclusion and
recursive glob semantics: [git-add](https://git-scm.com/docs/git-add),
[pathspec glossary](https://git-scm.com/docs/gitglossary#Documentation/gitglossary.txt-aiddefpathspecapathspec).
The disposable probe establishes the installed Git behavior; it does not prove
the complete sync path or every Git version. CI supplies the other platform.

Success means ordinary autonomous sync no longer needs intervention and leaves
no telemetry commit. The significant failure modes are silently swallowing a
real stage error, broadening the exclusion to user paths, losing edits during
recovery, or a later cleanup restoring the literal-prefix bug. Acceptance checks
target these outcomes rather than asserting argument strings.

## Scope and boundaries

Change the helper, regression coverage in `rust/loopflow/tests/sync_tests.rs`,
and the sync paragraph in `docs/ops.md` describing the existing checkpoint and
telemetry behavior. No schema, DTO, Desktop, Task-state or Graph-definition
changes. Preserve conflict handling introduced in #1748 / LOO-659.

Missing telemetry needs no special case. Telemetry-only working files must not
create a checkpoint. Other ignored files remain ignored. Genuine Git errors
propagate before checkpoint/merge, retaining whatever Git actually staged and
never reporting a clean checkout or completed sync.

Generated telemetry is normally untracked. The treatment of telemetry explicitly
staged or committed by a caller is an identified boundary: exclusion does not
remove an existing index entry. The bounded repair does not silently unstage
caller data or rewrite history; see `questions.md`. Acceptance must distinguish
this from preventing automatic inclusion of local telemetry.

LOO-312 / PR #1656 also edits `commands/ops/mod.rs` for onboarding. Keep its
independent changes during synchronization. LOO-661 owns credential persistence.
Infrastructure owns the single coordinated patch release, installation and
installed readback. This Task supplies the merged revision and remaining installed
verification; it does not start a competing publisher or install a candidate.

**Delete — do not maintain:** the literal-directory exclusion in
`stage_uncommitted_work` is replaced, with no fallback. No deletion targets
remain. Existing resolver, scratch-preservation and strategy tests survive.

**Forbidden outcomes:** force-adding ignored files; discarding staged/unstaged
work; swallowing `git add` errors; repository ignore configuration as a
prerequisite; excluding similarly named user directories; a parallel staging
helper; claiming source tests establish installed recovery of LOO-661.

## Remaining work and acceptance

The pathspec replacement and `sync_preserves_work_with_` CLI tests are implemented.
They cover ignored/unignored telemetry, staged and unstaged additions/edits,
deletions, renames, unusual names, neighboring paths, checkpoint contents,
upstream ancestry, retained telemetry bytes, unrelated ignored files, retry,
missing/telemetry-only state, and an index-lock failure retaining HEAD/index/work.
`docs/ops.md` states checkpoint behavior and the pre-staged-file boundary.

The shared fixture pushes only after upstream is ready. Content assertions check
both checkpoint and merge, then read each working file once; those states remain
separate preservation obligations. The direct staging helper needs no abstraction.

1. Gate runs the affected suites below, including the unchanged Graph sync
   preservation and conflict-resolver checks, plus formatting and Clippy.
2. Review the explicit pre-staged telemetry limitation in `questions.md`.
3. Deliver through the existing autonomous Graph. Record the exact merged
   revision; Infrastructure verifies a release containing it through installed
   sync before calling installed behavior proved.

Gate runs the affected suites once through the changed-aware runner; its Rust
plan includes formatting and Clippy. The additional suites below are required by
the Rust test guide for commit behavior. Expect every selected test to pass,
`cargo fmt` clean and `cargo clippy --all-targets -- -D warnings` clean.

```bash
uv run python scripts/test.py --reuse-passing --rust-filter 'test(sync_tests::) | test(agent_startup_tests::) | test(release_tests::) | test(release_lock_tests::)' < /dev/null
```

No display, native provider login or real network is part of acceptance. Do not
claim completion from the staging-only probe; the CLI fixtures cross index,
checkpoint, merge/recovery and telemetry consumers.

Check: 2026-10-27 `TMPDIR=/private/tmp uv run python scripts/test_network.py cargo test -p loopflow --test integration sync_preserves_work_with_ < /dev/null` — 4 passed; `cargo fmt` and `git diff --check` clean; affected suites and Clippy remain with gate.
