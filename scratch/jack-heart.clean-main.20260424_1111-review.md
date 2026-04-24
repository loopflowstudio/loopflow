# Review: sibling-worktree-safe default-branch sync

## What was implemented

- `sync_main()` now updates the checked-out default-branch worktree instead of only advancing `refs/heads/<default-branch>` when that branch is open in a sibling worktree.
- Dirty local work on that checked-out default-branch worktree is auto-stashed, hard-reset to `origin/<default-branch>`, then restored, including untracked files.
- `lf op pr` keeps the same flow but now benefits from the safer sync behavior when it refreshes the default branch before PR creation.
- Added regression coverage for the two failure cases that mattered in practice: stale sibling worktrees reporting phantom dirtiness, and dirty local edits on the default branch getting lost during sync.
- Updated `docs/lfop.md` and `docs/troubleshooting.md` to explain the sibling-worktree behavior and the recovery path.

## Key choices

- Reset the worktree that actually has the default branch checked out instead of calling `git update-ref` blindly. That keeps `HEAD`, the index, and the working tree aligned.
- Keep the direct `update-ref` fast path when the default branch is not checked out anywhere. No extra worktree churn when a simple ref move is safe.
- Stash with `--include-untracked` before resetting. The bug was about preserving real local work, not only tracked edits.
- Warn on stash failure or stash-pop failure instead of aborting the sync. The reset still repairs the stale worktree state, and a failed pop leaves the stash available for manual recovery.

## How it fits together

`sync_main()` now resolves whether the default branch is checked out in the current repo or another worktree. If it is, `reset_worktree_to()` performs a stash/reset/pop cycle inside that worktree. If it is not, Loopflow keeps using `git update-ref` to fast-forward the local branch ref. `lf op pr`, `lf op next`, `lf op rebase`, and other callers inherit the safer behavior without changing their own control flow.

## Risks and bottlenecks

- `stash pop` can still conflict if local edits overlap with upstream changes. The code warns and preserves the stash rather than silently discarding work, but recovery is still manual.
- The docs describe the default-branch behavior; if future changes special-case non-`origin` remotes, these docs will need another pass.
- No branch design doc was present under `scratch/`, so validation used the Rust checks and regression tests directly.

## What's not included

- No change to merge, rebase, or PR UX beyond the safer underlying sync.
- No new CLI flags or config knobs.
- No attempt to auto-resolve stash-pop conflicts.

## Validation

- `cargo fmt --check`
- `cargo clippy -- -D warnings`
- `cargo test --all`
