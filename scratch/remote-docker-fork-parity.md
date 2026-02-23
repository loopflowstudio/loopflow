# 01E: Docker Fork Parity

## Problem

Fork flows (`wave-reduce`, `wave-polish`, `wave-expand`) currently fail immediately under the Docker executor with `fork is not supported by the docker executor yet`.

This blocks remote users (Docker is the default remote runtime) from the same core workflow that works in native mode: branch fan-out, synthesis, and fail-late reporting.

Why now:
- Phase 01D established fork semantics as non-negotiable correctness behavior.
- Remote phase quality depends on parity between native and Docker execution paths.
- Current behavior forces users to avoid built-in fork flows in remote mode.

## Approach

Implement Docker fork execution as a first-class path in `WaveExecutor::run_fork`, reusing native semantics but mapping each branch to an isolated Docker worktree.

Execution model:
1. **Plan branches once** using existing `plan_fork_execution` logic.
2. **Prepare per-branch workspace identity** (`run_id + branch_index`) so each fork branch maps to a unique Docker worktree path.
3. **Serialize git mutations only** (shared-clone fetch/worktree add/reset) with the existing repo-scoped mutation lock.
4. **Launch branch agents in parallel** (one container per branch) with branch-specific worktree mounts and prefixed logs.
5. **Persist fork branch outcomes** to fork-run records and `.lf/fork-manifest.json`.
6. **Run `synthesize` after branch completion** using manifest data.
7. **Fail late**: mark run failed at end if any branch failed or synthesize failed.
8. **Always cleanup** branch worktrees + fork records, including recovery-safe cleanup after daemon restart.

Research applied:
- Reuse existing native fork contract (`manifest -> synthesize -> cleanup -> fail-late`) rather than inventing Docker-only semantics.
- Reuse existing Docker shared-clone + lock strategy to avoid git ref/worktree races.
- Follow CI matrix fail-late pattern: collect all branch results before final run verdict.

## Implementation design

### Core change: derive container worktree from `cwd`

`DockerExecutor::run()` currently derives the container worktree path from `wave_id + wave_run_id` via `resolve_workspace`. This assumes 1:1 between wave run and container worktree. Fork breaks this — multiple agents share one wave run but need distinct workspaces.

Fix: split `resolve_workspace` into shared infra (volume, shared clone, repo source — from wave/run) and per-agent worktree (from `cwd`). The container worktree path becomes a function of the host `cwd`, not the wave run branch.

`DockerWorkspace` fields that stay the same across fork branches and parent:
- `volume` — per-repo, not per-branch
- `repo_source` — same host repo
- `container_shared_clone` — all branches fork from the same clone
- `has_remote` — property of the repo

Fields that differ per fork branch:
- `container_worktree` — derived from `cwd`
- `branch` — each branch gets its own git branch name

This also benefits sidecars and any future case where multiple agents share a wave run.

### Volume layout

Flat siblings matching the host-side convention:

```
/workspace/repos/<repo>/
  main/                          # shared clone
  worktrees/
    my-wave/                     # main run worktree
    my-wave-fork-0/              # fork branch 0 — full git worktree
    my-wave-fork-1/              # fork branch 1 — full git worktree
```

The synthesize container mounts the same volume and can read all branch worktrees directly.

### Host-side fork directories

Native: `run_fork` creates real git worktrees via `create_worktree`.
Docker: `run_fork` creates empty directories via `mkdir`. The actual git worktree lives in the Docker volume. The host directory exists only as a sync target for `sync_to_host_worktree`.

### Manifest writing: `write_to_workspace` on `AgentExecutor`

Problem: `run_fork` writes `.lf/fork-manifest.json` to the host main worktree. Then synthesize calls `DockerExecutor::run()`, which calls `prepare_workspace`, which syncs container→host — overwriting the manifest. There's no host→container sync path.

Fix: add a `write_to_workspace` method to `AgentExecutor`:

```rust
async fn write_to_workspace(
    &self,
    cwd: &Path,
    relative_path: &str,
    content: &[u8],
) -> Result<()> {
    // Default: write to host filesystem
    let path = cwd.join(relative_path);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, content)?;
    Ok(())
}
```

Docker overrides to also write into the container volume via a helper container. The manifest ends up in both host and container. Synthesize (running in the container) reads it directly.

Branch worktree paths in the manifest are container-side paths for Docker, host-side paths for native.

### Cleanup

Native: `remove_worktree()` on host paths (existing behavior).
Docker: helper containers to remove volume-internal worktrees via `git worktree remove`, same pattern as `cleanup_wave`. Fork-run records deleted from store regardless of executor.

### What stays the same

- `plan_fork_execution` — unchanged
- `ForkBranchExecution` / `ForkManifestBranch` — unchanged
- `ForkRun` / `ForkRunStatus` store operations — unchanged
- Orphaned fork cleanup on startup recovery — unchanged
- Synthesize step execution via `run_step` — unchanged
- Fail-late semantics — unchanged
- Scheduler slot acquisition per branch — unchanged

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Run fork branches sequentially in one Docker worktree | Lowest implementation risk, but very slow and not true fork behavior | Violates fork intent and diverges from native parallel semantics |
| Keep rejecting fork for Docker and document limitation | No engineering effort now | Leaves a core remote capability gap and blocks wave flows users already rely on |
| Reuse one shared worktree with hard resets between branches | Fewer worktrees/paths | Branch contamination and race risk under parallel execution; hard to make restart-safe |
| Add fork-specific field to `AgentRunContext` | Explicit fork identity per agent | Too specific; the real constraint to relax is 1:1 wave-run-to-workspace, which `cwd` already solves generally |
| Docker-specific branch in `run_fork` for manifest writing | No trait changes | Couples `run_fork` to `DockerExecutor` internals via downcast; breaks if new executor types appear |

## Key decisions

- **Derive container worktree from `cwd`**, not from wave run lookup. Relaxes the 1:1 wave-run-to-workspace assumption for fork, sidecars, and any future multi-agent case.
- **Flat sibling volume layout** (`.../worktrees/<wave>-fork-<N>`) matching host convention. No nested `/<run>/<branch>` paths.
- **`write_to_workspace` on `AgentExecutor`** for inter-step file handoff. Docker writes to both host and container volume. Avoids Docker-specific code in `run_fork`.
- **Lock only shared-clone mutation windows**; keep agent execution parallel to preserve fork throughput.
- **Treat cleanup as mandatory and idempotent**: volume-side worktree removal via helper containers, plus fork-run record deletion.
- **Keep this PR scoped** to fork parity only (no Docker runtime redesign, no cross-host sync changes).

## Scope

- In scope:
  - Refactor `DockerExecutor` workspace derivation to use `cwd`
  - Add `write_to_workspace` to `AgentExecutor` trait
  - Remove Docker guard in `run_fork`
  - Docker fork branch worktree isolation (mkdir on host, git worktree in volume)
  - Manifest writing through `write_to_workspace`
  - Parallel branch container execution
  - Fail-late run status and error messaging parity
  - Volume-side fork cleanup via helper containers
  - Cleanup/recovery parity for fork artifacts
  - Targeted tests for success, partial failure, timeout, cleanup
- Out of scope:
  - Host→container sync (beyond `write_to_workspace`)
  - Broader Docker executor architecture changes
  - Non-fork executor feature work

## Done when

- Running fork flows with `executor.type = docker` no longer fails up front.
- For Docker fork runs, all branches execute, manifest is written, synthesize runs, and final status is fail-late.
- Synthesize agent can read full git checkout of each fork branch worktree from the volume.
- Fork worktrees and fork-run records are cleaned up on success, failure, timeout, and restart recovery.
- Verification passes:
  - `cargo test -p loopflow fork_`
  - `cargo test -p loopflow docker_`
  - CI `docker-smoke` job remains green.
