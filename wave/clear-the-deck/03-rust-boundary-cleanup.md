# 03: Rust Boundary Cleanup

**Finish line:** `lf` no longer imports workspace file/worktree cleanup helpers from `lfd::executor`, and branch-existence checks live in one shared helper.

## Carried context

- `rust/loopflow/src/lf/commands/flow.rs` still imports `cleanup_workspace_worktree`, `remove_workspace_file`, and `write_workspace_file` from `crate::lfd::executor`, even though the flow runner is not part of the daemon layer.
- Those helpers currently live in `rust/loopflow/src/lfd/executor/mod.rs` only because Docker and daemon code also use them.
- `rust/loopflow/src/ops/combine.rs` keeps a private `branch_exists()` while `rust/loopflow/src/engine/worktrees.rs` already owns the shared branch-existence check used elsewhere.

## What to build

1. Move the workspace file/worktree helpers into the shared engine layer, or another dependency direction that both `lf` and `lfd` can use without crossing layers.
2. Update the flow runner and Docker executor to call that shared home instead of reaching through `lfd::executor`.
3. Delete the duplicate `branch_exists()` in `ops/combine.rs` and reuse the shared implementation.
4. Keep or add tests around worktree cleanup so the move shrinks the dependency graph without changing behavior.

## Uncertainty

- The right shared home may be `engine/worktree.rs`, `engine/worktrees.rs`, or a new engine helper module; pick the smallest place that matches the concept instead of inventing a generic utility bucket.
- The cleanup helper currently falls back to `remove_dir_all` when `.git` is absent. Preserve that behavior unless a test proves it is wrong.

## Done when

- No `lf` module imports helper functions from `lfd::executor`.
- Branch existence is implemented once and reused by combine/next/worktree flows.
- Worktree helper tests still prove the shared behavior after the move.
