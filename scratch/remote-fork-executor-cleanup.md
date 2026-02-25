# Fork Executor Cleanup

Follow-up improvements after 01E (Docker fork parity) shipped. Ordered by dependency chain — each priority unblocks the next.

## Priority 1: Deduplicate fork constants and path computation

One manifest path constant in `engine::fork`, exported and used by both CLI and daemon. One `fork_worktree_path` function with a consistent naming convention (`-fork-N` suffix). One `is_ephemeral_worktree_path` check that matches the single convention. Mechanical, low-risk, removes a class of drift bugs.

## Priority 2: Thread branch name through AgentRunContext

Add an optional `branch` field to `AgentRunContext`. Fork orchestration fills it. Docker uses it when present, falling back to filesystem inference only for recovery/reattach. This collapses `infer_fork_branch_from_worktree`, simplifies `resolve_workspace_branch` to a one-liner, and eliminates the dummy `"workspace"` wave_run_id in `resolve_workspace_for_host_worktree`.

## Priority 3: Unify CLI fork execution through executor trait

Once constants and paths are shared, evaluate whether the CLI fork path should use `write_to_workspace` and `cleanup_ephemeral_worktree` through a `LocalProcessExecutor`. The daemon path already does this. If the CLI path can call the same trait methods, the direct-filesystem functions in `engine::fork` (`write_fork_manifest`, `cleanup_fork_worktrees`) become dead code. Don't introduce a new `ForkRunner` abstraction — the executor trait already provides the right hook points.

## Priority 4: Smaller cleanups

- Deduplicate `cleanup_host_worktree` and `cleanup_ci_fix_worktree`.
- Move wave lifecycle hooks into the executor trait to remove `executor_type` branching from HTTP routes.
- Simplify Docker file-ops workspace resolution once branch threading is in place.

## Leave alone

- Fork planning (`plan_fork_execution`, `merge_directions`)
- Manifest data structures (`ForkManifest`, `ForkManifestBranch`)
- Scheduler slot model
- Docker mutation locks and container rehydration
