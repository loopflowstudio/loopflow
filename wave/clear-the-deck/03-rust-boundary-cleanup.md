# 03: Rust Boundary Cleanup

**Finish line:** `lf` no longer imports workspace file/worktree cleanup helpers from `lfd::executor`, and branch/ref existence checks live in one shared helper.

## Carried context

- `rust/loopflow/src/lf/commands/flow.rs` still imports `cleanup_workspace_worktree`, `remove_workspace_file`, and `write_workspace_file` from `crate::lfd::executor`, even though the flow runner is not part of the daemon layer.
- Those helpers currently live in `rust/loopflow/src/lfd/executor/mod.rs` only because Docker and daemon code also use them.
- `rust/loopflow/src/ops/combine.rs` keeps a private `branch_exists()` that checks both local and `origin/*` refs, while `rust/loopflow/src/engine/worktrees.rs` already owns the shared branch-existence check used elsewhere but only looks at local heads.

## What to build

1. Move the workspace file/worktree helpers into the shared engine layer, or another dependency direction that both `lf` and `lfd` can use without crossing layers.
2. Update the flow runner and Docker executor to call that shared home instead of reaching through `lfd::executor`.
3. Delete the duplicate branch/ref existence helper in `ops/combine.rs` by reusing or widening the shared implementation instead of keeping parallel logic.
4. Keep or add tests around worktree cleanup and branch/ref detection so the move shrinks the dependency graph without changing behavior.

## Uncertainty

- The right shared home may be `engine/worktree.rs`, `engine/worktrees.rs`, or a new engine helper module; pick the smallest place that matches the concept instead of inventing a generic utility bucket.
- The cleanup helper currently falls back to `remove_dir_all` when `.git` is absent. Preserve that behavior unless a test proves it is wrong.
- `combine` currently cares about remote refs too, so do not blindly swap in `engine::worktrees::branch_exists()` unless the shared helper grows that behavior or `combine` is intentionally narrowed.

## Done when

- No `lf` module imports helper functions from `lfd::executor`.
- Branch/ref existence rules are implemented once and reused by combine/next/worktree flows.
- Worktree helper tests still prove the shared behavior after the move.
