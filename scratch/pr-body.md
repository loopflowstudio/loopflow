## Try it!

```bash
cargo test --test git_tests sync_main_from_feature
cargo fmt --check
cargo clippy -- -D warnings
cargo test --all
```

The targeted git tests cover the two regressions: syncing from a feature worktree no longer leaves the default-branch worktree phantom-dirty, and dirty local edits on that checked-out default branch survive the sync.

## Intent

Make Loopflow's default-branch sync safe when the branch is checked out in a sibling worktree. `lf op pr`, `lf op sync`, `lf op next`, and other flows should be able to refresh upstream state without leaving the main worktree out of sync with its ref or discarding local edits.

## Assumptions

- The repo's canonical upstream is `origin/<default-branch>`.
- If `stash pop` conflicts after the reset, preserving the stash for manual recovery is better than failing the sync before repairing the stale worktree state.
- Documentation for `lf op sync` and worktree troubleshooting is the right place to explain this behavior to users.

## Key decisions

- Reset the worktree that actually has the default branch checked out instead of moving the ref with `git update-ref` alone.
- Keep the `update-ref` fast path when the default branch is not checked out anywhere.
- Include untracked files in the temporary stash so ad hoc local work survives the reset.

## Not included

- Automatic conflict resolution when restoring stashed changes.
- Any new CLI surface area.
