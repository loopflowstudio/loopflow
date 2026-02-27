# 01: Rust Improvements

**Finish line:** `lf ops wt list` shows `landed-dirty` (red) and `remote-gone` (yellow) states. `lf ops wt prune --force` removes remote-gone clean worktrees.

## What to build

### WorktreeState changes

Add two fields to `WorktreeState` in `engine/worktrees.rs`:

```rust
pub dirty: bool,        // uncommitted changes in worktree
pub remote_gone: bool,  // remote branch no longer exists
```

Populate `dirty` from `git status --porcelain` (already checked elsewhere — centralize into the struct). Populate `remote_gone` from `git ls-remote --heads origin <branch>` returning nothing. Skip `remote_gone` detection if the command fails (offline).

### wt_list changes

- Show `landed-dirty` status when `merged && dirty`. Red marker. Replaces showing `merged` + `dirty` independently for this case.
- Show `remote-gone` status when `remote_gone && !merged`. Yellow marker.
- Existing `merged` and `prunable` display unchanged for non-dirty merged worktrees.

### wt_prune changes

- Include `remote_gone && !dirty && !has_unpushed_beyond_main` worktrees as prunable.
- `--force` removes remote-gone clean worktrees alongside merged ones.
- Dry-run output groups by reason: "merged", "remote-gone", "empty".

### JSON output

`wt list --format json` should include `dirty` and `remote_gone` fields so the agent step can consume structured data.

## Done when

1. `WorktreeState` has `dirty` and `remote_gone` fields
2. `wt list` displays `landed-dirty` and `remote-gone` states
3. `wt prune --force` removes remote-gone clean worktrees
4. `cargo test --all` passes
5. `cargo clippy -- -D warnings` passes
