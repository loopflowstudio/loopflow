# Docker Fork Parity — Design Review

## What was implemented

Fork flows (`wave-reduce`, `wave-polish`, `wave-expand`) now execute under the Docker executor with the same semantics as native: parallel branch execution, manifest handoff to `synthesize`, fail-late final status, and cleanup of fork artifacts. The Docker executor no longer rejects fork runs.

Three new `AgentExecutor` trait methods abstract workspace file operations so fork orchestration doesn't downcast to Docker:

- `write_to_workspace(cwd, relative_path, content)` — writes to host, and in Docker also copies to the container volume
- `remove_from_workspace(cwd, relative_path)` — removes from host, and in Docker also removes from volume
- `cleanup_ephemeral_worktree(repo, worktree)` — in Docker, removes the container-side worktree before cleaning the host

Docker workspace resolution was changed from wave-name-based to host-worktree-path-based, so multiple concurrent fork branches each get their own container worktree.

The `research` step was also removed from wave-reduce/polish/expand flows (now fork → publish directly).

## Key choices

**Executor trait hooks vs Docker downcasts.** Fork orchestration in `wave/fork.rs` calls trait methods for file ops and cleanup. The alternative was matching on `executor_type` and calling Docker-specific code, which would couple fork logic to the executor implementation. Trait hooks keep it clean and make future executors (e.g., remote SSH) get fork support for free.

**Host worktree path as workspace identity.** Docker workspace resolution now uses `cwd` (the host worktree path) to derive the container worktree slug. Previously it used wave name, which meant all fork branches would collide in the same container worktree. Using host path gives each branch a unique container worktree.

**Index-based channel instead of label-based HashMap.** Fork results are collected in a `Vec<Option<i32>>` indexed by branch position, replacing a `HashMap<String, Result<i32>>`. This preserves branch order for manifest construction and simplifies error handling — errors are resolved at the send site (exit code 1) rather than propagated through the channel.

**Host-side git worktrees for prompt assembly.** Prompt assembly (`build_step_prompt`) still runs against host-side git worktrees. The alternative — building prompts from container-side files — would require either pre-prompt container→host sync or a prompt build path that doesn't depend on host worktree materialization. The current approach works because prompt assembly happens before container launch and worktrees are cleaned up after.

## How it fits together

```
Fork orchestration (wave/fork.rs)
  ├── create_worktree() — host-side git worktrees for prompt assembly
  ├── build_step_prompt() — reads step/context from host worktree
  ├── launch_agent() → AgentExecutor::run() — Docker creates container worktree from volume
  ├── AgentExecutor::write_to_workspace() — manifest to host + container volume
  ├── run_step(synthesize) — reads manifest from container worktree
  └── cleanup_fork()
        ├── AgentExecutor::remove_from_workspace() — manifest cleanup
        ├── AgentExecutor::cleanup_ephemeral_worktree() — volume + host cleanup
        └── store.delete_fork_runs() — database cleanup
```

Volume layout:
```
/workspace/repos/<repo>/
  main/                    ← shared clone
  worktrees/
    <wave>                 ← main wave worktree
    <wave>-fork-0          ← branch 0
    <wave>-fork-1          ← branch 1
```

## Risks and bottlenecks

**Host-side worktree dependency.** Fork branches create real git worktrees on the host for prompt assembly. If host disk is tight or many forks run concurrently, this could be a bottleneck. Cleanup is idempotent but happens after the fork completes, not incrementally.

**Branch inference from path.** `infer_fork_branch_from_worktree` parses `-fork-N` from the host path name. This is fragile if the naming convention changes. The follow-up doc (Priority 2) proposes threading branch name through `AgentRunContext` to eliminate this.

**Helper container overhead.** `write_file_to_volume` and `remove_file_from_volume` each spawn short-lived helper containers. For forks with many branches, this adds container creation overhead. In practice, it's only the manifest write/remove (2 operations per fork), not per-branch.

## What's not included

- **Deduplication of fork constants** — `FORK_MANIFEST_RELATIVE_PATH` is defined in both `engine::fork` and `executor::wave::fork`. The follow-up doc (`fork-executor-cleanup.md`) tracks this as Priority 1.
- **Branch name threading through context** — Would eliminate path-based branch inference. Priority 2 in follow-up.
- **CLI fork execution through executor trait** — Could unify CLI and daemon fork paths. Priority 3 in follow-up.
- **Broader Docker runtime redesign** — Explicitly out of scope.
