# Remote Fork Executor Cleanup — Review

## What was implemented

- Canonicalized fork shared contract in `engine::fork`:
  - Exported `FORK_MANIFEST_RELATIVE_PATH`.
  - Standardized `fork_worktree_path()` to `-fork-N` naming.
  - Removed direct manifest write/cleanup helpers from `engine::fork`.
- Switched CLI fork artifact operations to executor workspace hooks:
  - Manifest write via `write_to_workspace`.
  - Manifest removal via `remove_from_workspace`.
  - Fork worktree cleanup via `cleanup_ephemeral_worktree`.
- Threaded explicit branch identity through agent execution:
  - Added `branch: Option<&str>` to `AgentRunContext`.
  - Added `branch: Option<String>` to `AgentLaunchRequest` and wired all launch callsites.
  - Docker normal execution now resolves branch by precedence: context branch → checked-out git branch → fallback branch.
  - Recovery-only path keeps fork-branch inference from filesystem when full context is unavailable.
- Consolidated executor workspace lifecycle hooks:
  - Added `ensure_wave_workspace` and renamed `cleanup_wave` to `cleanup_wave_workspace` on `AgentExecutor`.
  - Removed HTTP route branching on `executor_type` for create/delete wave workspace side effects.
- Deduplicated worktree cleanup helpers:
  - Unified cleanup behind `cleanup_workspace_worktree`.
  - Reused it for CI-fix worktree cleanup.
- Tightened ephemeral worktree detection to numeric `-fork-<digits>` suffix and added unit tests.
- Added assertion coverage that fork/synthesize launches propagate expected branch context into runner calls.
- Wave planning artifact update from ingest:
  - Moved `wave/remote/fork-executor-cleanup.md` to `scratch/remote-fork-executor-cleanup.md`.
  - Removed `wave/remote/06-remote-file-access.md` from the wave queue on this branch.

## Key choices

- **Explicit branch context over path inference** in normal execution to avoid incorrect Docker workspace branch mapping.
- **Single fork naming convention** (`-fork-N`) across CLI, daemon, Docker, and janitor matching.
- **Executor trait as workspace mutation boundary** for fork artifacts, avoiding duplicate filesystem codepaths.
- **Kept path inference only for recovery** where execution context may not be present.

## How it fits together

Fork planning still happens once, but both CLI and daemon now use the same fork constants/path builder and write/cleanup fork artifacts through executor hooks. Wave launch now carries branch identity into `AgentRunContext`, and Docker workspace resolution consumes that identity directly for normal runs. HTTP wave lifecycle endpoints delegate workspace create/cleanup to executor trait methods rather than branching on executor type.

## Risks and bottlenecks

- CLI fork cleanup now runs sequentially through executor hooks; large fork counts may clean up slightly slower than previous threaded cleanup.
- Fork naming migration is intentionally breaking for legacy `.fork-N` paths.
- Startup/recovery behavior still depends on filesystem-based inference when context is missing.

## What's not included

- No changes to fork planning semantics (`plan_fork_execution`, `merge_directions`).
- No manifest schema changes (`ForkManifest`, `ForkManifestBranch`).
- No scheduler model changes.
- No Docker rehydration lock model changes.

## Validation run

- `cargo fmt --all -- --check`
- `cargo clippy -p loopflow --all-targets -- -D warnings`
- `cargo test -p loopflow fork_worktree_path`
- `cargo test -p loopflow resolve_workspace_branch`
- `cargo test -p loopflow execute_fork_`
- `cargo test -p loopflow execute_fork_merges_directions_and_prefixes_branch_logs`
- `cargo test -p loopflow is_ephemeral_worktree_path`

Additional check:
- `cargo test -p loopflow` was run; 2 Docker startup tests failed in this environment due missing `/var/run/docker.sock`.
