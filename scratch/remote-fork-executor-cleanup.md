# Fork Executor Cleanup

## Status

Completed on branch `jack-heart.remote.20260224_2049`.

This document is the canonical summary for this work item. It replaces earlier split planning/review notes.

## Goal

Keep fork execution behavior consistent across CLI, daemon, and Docker by enforcing one shared fork contract (paths, branch identity, and workspace lifecycle handling).

## Implemented

- Canonicalized fork contract in `engine::fork`:
  - Exported `FORK_MANIFEST_RELATIVE_PATH`.
  - Standardized `fork_worktree_path()` to `-fork-N` naming.
  - Removed direct manifest write/cleanup helpers from `engine::fork`.
- Routed CLI fork artifact operations through executor hooks:
  - Manifest write: `write_to_workspace`.
  - Manifest remove: `remove_from_workspace`.
  - Worktree cleanup: `cleanup_ephemeral_worktree`.
- Threaded explicit branch identity through execution:
  - `AgentRunContext` now carries `branch: Option<&str>`.
  - `AgentLaunchRequest` now carries `branch: Option<String>`.
  - Docker normal execution resolves branch by precedence:
    1. explicit context branch,
    2. checked-out git branch,
    3. fallback branch.
  - Filesystem-based branch inference is retained only for recovery paths without full context.
- Unified workspace lifecycle behavior behind executor trait methods:
  - Added `ensure_wave_workspace`.
  - Renamed `cleanup_wave` to `cleanup_wave_workspace`.
  - Removed HTTP route branching on `executor_type` for wave workspace side effects.
- Deduplicated worktree cleanup helpers behind shared `cleanup_workspace_worktree`.
- Tightened ephemeral worktree detection to `-fork-<digits>` suffix and added unit tests.
- Added launch assertion coverage ensuring branch context propagates into runner calls.
- Updated wave planning artifacts:
  - Moved `wave/remote/fork-executor-cleanup.md` into `scratch/remote-fork-executor-cleanup.md`.
  - Removed `wave/remote/06-remote-file-access.md` from this branch’s wave queue.

## Key decisions

- Prefer explicit branch context over filename/path inference during normal execution.
- Enforce one fork naming convention (`-fork-N`) across CLI, daemon, Docker, and janitor logic.
- Treat `AgentExecutor` as the workspace mutation boundary for fork artifacts.
- Do not keep active compatibility shims for legacy `.fork-N` names.

## Scope boundaries

Out of scope (unchanged):

- Fork planning semantics (`plan_fork_execution`, `merge_directions`)
- Manifest schema (`ForkManifest`, `ForkManifestBranch`)
- Scheduler slot behavior
- Docker rehydration/mutation lock model

## Validation

Executed:

- `cargo fmt --all -- --check`
- `cargo clippy -p loopflow --all-targets -- -D warnings`
- `cargo test -p loopflow fork_worktree_path`
- `cargo test -p loopflow resolve_workspace_branch`
- `cargo test -p loopflow execute_fork_`
- `cargo test -p loopflow execute_fork_merges_directions_and_prefixes_branch_logs`
- `cargo test -p loopflow is_ephemeral_worktree_path`

Additional run:

- `cargo test -p loopflow` (2 Docker startup tests failed in this environment due missing `/var/run/docker.sock`)

## Residual risk

- Fork cleanup now goes through executor hooks sequentially; very large fork counts may clean up slightly slower than previous threaded cleanup.
- Recovery behavior still relies on filesystem inference when run context is unavailable.
