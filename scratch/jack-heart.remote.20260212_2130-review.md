# Design Review: Split executor, extract fork module, harden remote execution

## What was implemented

This branch restructures the daemon executor and CLI fork execution across 26 files (+3046/-1966 lines):

1. **Executor split**: Broke the monolithic `executor.rs` (~3700 lines) into five focused modules:
   - `mod.rs` — `AgentExecutor` trait, `StartupRecovery`, stream processing helpers
   - `docker.rs` — Docker container lifecycle, volumes, images, credential mounts
   - `wave.rs` — Wave and fork orchestration (step execution, branch progression, PR creation)
   - `helpers.rs` — Prompt building, branch operations, worktree management
   - `local.rs` — Direct subprocess executor backend

2. **Fork module extraction**: Moved fork-related data structures and operations from `lf/commands/flow.rs` to `engine/fork.rs`. Shared types (`ForkManifest`, `ForkManifestBranch`) and functions (`fork_worktree_path`, `merge_directions`, `cleanup_fork_worktrees`) are now in the engine layer, usable by both CLI and daemon.

3. **Parallel fork execution in CLI**: `lf flow` now runs fork branches in parallel using OS threads and sibling worktrees (`repo.fork-N`). Writes a `fork-manifest.json` for downstream synthesis.

4. **Remote execution hardening**:
   - `prepared_key` TTL (5-minute expiry) prevents stale workspace reuse
   - `CancellationToken`-based fork abort prevents scheduler slot leaks
   - Async context gathering via `spawn_blocking` keeps the executor thread free
   - Parallel fork cleanup in both CLI (scoped threads) and daemon (concurrent tasks)
   - Wave progression deduplication reduces repeated bookkeeping

5. **Docker setup**: New `install-loopflow.sh` baseline contract for agent containers and `env-setup.sh` for dev environment tooling.

## Key choices

**Sibling worktrees for CLI forks**: Fork branches use `repo.fork-N` directories alongside the main worktree rather than nested directories. This avoids git worktree nesting issues and makes parallel execution straightforward. Cleaned up after synthesis.

**Engine-level fork types**: `ForkManifest` and helpers live in `engine/` rather than `lf/` because the daemon also needs fork manifest awareness. One source of truth for fork data structures.

**`CancellationToken` over `JoinHandle::abort()`**: Fork tasks check the cancellation token before acquiring scheduler slots. This prevents slot leaks that occurred when `abort()` interrupted a task between slot acquisition and release.

**`String` error type on `DockerCredentialMount::from_config`**: Private helper used in exactly one place where the caller maps into `anyhow!`. Converting to `anyhow::Result` would add a dependency for no gain since the error message is already descriptive.

## How it fits together

```
engine/fork.rs         — shared fork types + cleanup (CLI and daemon)
engine/worktree.rs     — worktree create/remove/info (used by fork.rs)
lf/commands/flow.rs    — CLI: thread-based parallel fork execution
lfd/executor/wave.rs   — daemon: tokio-based parallel fork execution
lfd/executor/docker.rs — container lifecycle, volumes, credentials
lfd/executor/mod.rs    — AgentExecutor trait, stream helpers
```

The engine provides data structures and worktree operations. CLI and daemon each implement their own execution strategy (threads vs tokio tasks) but share the same manifest format and cleanup logic.

## Risks and bottlenecks

**No integration tests for parallel fork execution.** Unit tests cover helpers (worktree paths, manifest serialization, direction merging, credential resolution) but the full fork path—parallel worktree creation, thread/task spawning, result aggregation, cleanup on failure—is only tested manually. This is the highest-risk code on the branch.

**CLI fork cleanup on partial failure.** If worktree creation succeeds for 3 of 5 branches then fails, the cleanup path removes created worktrees but the error message could be clearer about which branches ran.

**Docker CLI dependency.** Image builds shell out to `docker build` rather than using the Bollard API. This means Docker CLI must be in PATH on the host, which is an assumption that isn't validated at daemon startup.

## What's not included

- `ForkSelect::One` and `ForkSelect::Prompt` in the CLI (daemon only)
- Fork log prefixing to distinguish concurrent branch output
- Integration tests for end-to-end fork execution
- Engine simplification items documented in `roadmap/rust/engine-simplification.md` (deferred)
