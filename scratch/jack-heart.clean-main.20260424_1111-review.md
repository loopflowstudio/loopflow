# Review: safe default-branch sync across sibling worktrees

## What was implemented

Loopflow now syncs the checked-out default-branch worktree instead of only moving `refs/heads/<default-branch>` when that branch is open in a sibling worktree. The branch keeps local edits by auto-stashing before the reset, restoring them after the reset, and preserving the stash if restore conflicts need manual recovery. The user-facing docs now explain the behavior in `docs/lfop.md`, `docs/troubleshooting.md`, and the README sync flow description.

## Key choices

- Reset the worktree that actually has the default branch checked out so `HEAD`, the index, and the working tree stay aligned.
- Keep the old `update-ref` fast path when the default branch is not checked out anywhere.
- Include untracked files in the temporary stash so ad hoc local work survives the sync.
- Leave the stash in place on restore conflicts instead of aborting before the stale worktree is repaired.

## How it fits together

`sync_main()` now looks for the worktree that has the default branch checked out. If it finds one, it runs a reset-and-restore flow in that worktree; otherwise it falls back to `git update-ref`.

The regression coverage in `rust/loopflow/tests/git_tests.rs` proves both outcomes that matter to users: syncing from a feature worktree no longer leaves the default-branch worktree phantom-dirty, and dirty local edits on that default branch survive the sync.

## Risks and bottlenecks

- `stash pop` can still conflict after the reset. The code preserves the stash for manual recovery, but the user has to finish the merge.
- The behavior assumes `origin/<default-branch>` is the canonical upstream source of truth.
- The docs are intentionally concise; if users keep hitting stash-recovery questions, `docs/troubleshooting.md` may need a fuller recovery recipe.

## What's not included

- Automatic conflict resolution for stashed changes.
- New CLI flags or alternate sync modes.
- Non-Rust validation suites; this branch only changes Rust and docs.

## Validation

- `cargo test --test git_tests sync_main_from_feature`
- `cargo fmt --check`
- `cargo clippy -- -D warnings`
- `cargo test --all`
