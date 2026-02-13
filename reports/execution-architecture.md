# Execution Architecture

Loopflow's execution stack has three clean layers with one-way dependencies:

```
engine/  (shared library: data models, flow loading, agent commands, worktree ops)
   ↑              ↑
   │              │
lf/            lfd/
(CLI)          (daemon)
```

Neither CLI nor daemon imports the other. Both consume the same `engine::` types (`Flow`, `ConcreteItem`, `FlowAction`, `ForkSelect`, `Config`). This separation is well-maintained.

## Architecture

**engine/** — Pure data structures and algorithms. Loads flows from YAML, expands them into `ConcreteItem` sequences (steps + forks), and provides `next_action()` to drive execution. Also handles worktree creation/removal, agent command building for 4 backends (Claude, Codex, Gemini, OpenCode), and config loading/merging.

**lf/commands/flow.rs** — CLI flow runner. Synchronous, thread-based. Creates sibling worktrees for fork branches, spawns OS threads for parallel execution, writes a manifest to `.lf/fork-manifest.json`, and cleans up after synthesis. Only supports `ForkSelect::All`.

**lfd/executor/** — Daemon flow executor, split into modules: `mod.rs` (trait + startup recovery), `docker.rs` (container lifecycle, volumes, images, credentials), `wave.rs` (wave/fork orchestration), `helpers.rs` (prompt building, branch ops), `local.rs` (direct subprocess backend). Async (tokio), supports all three `ForkSelect` modes.

**lfd/config.rs** — Daemon-specific config. Loads from `~/.lf/lfd.yaml`, supports `local` and `docker` executor types, enforces an allowlist for credential mounts (claude, codex, gemini, gitconfig, ssh, gnupg).

## Data Flow

A flow execution follows this path:

```
YAML (steps/*.md, flows/*.yaml)
  → load_flow() → expand_flow() → Vec<ConcreteItem>
  → next_action(items, index) → FlowAction
  → match FlowAction:
      RunStep    → build_agent_command() → subprocess or container
      Fork(All)  → create worktrees → parallel execution → synthesize → cleanup
      Complete   → finalize
```

Fork execution diverges between CLI and daemon:

| Concern | CLI (lf/) | Daemon (lfd/) |
|---------|-----------|---------------|
| Parallelism | OS threads | tokio tasks |
| Worktrees | Sibling dirs (`repo.fork-N`) | Docker volume worktrees |
| Persistence | Manifest JSON file | SQLite/Postgres store |
| Recovery | None | Container rehydration |
| Select modes | All only | All, One, Prompt |

## Key Abstractions

**`AgentExecutor` trait** — The core abstraction for running agents. Two implementations: `LocalProcessExecutor` (direct subprocess) and `DockerExecutor` (container-based with volume management, image lifecycle, credential isolation, and startup recovery).

**`KeyedLocks`** — Custom per-key mutex manager in the Docker executor. Serializes mutations to the same shared clone (repo-level locks) and prevents concurrent duplicate image builds (image-level locks).

**`RepoVolumeIdentity`** — Deterministic naming for Docker volumes and images. SHA256 of canonical repo URL or local path. Prevents cross-repo collisions.

**`ForkManifest` / `ForkBranchTask`** — CLI-side fork tracking. Records branch metadata, worktree paths, and exit codes for downstream synthesis.

## Tensions

- **CLI vs daemon fork parity**: CLI supports only `ForkSelect::All` while the daemon supports all three modes. The CLI returns a hard error for One/Prompt. This is a deliberate constraint, not a bug — One/Prompt require interactive state or user prompting that the CLI's synchronous model doesn't support well. But it means flows that use `select: one` or `select: prompt` can only run via `lfd`.

- **Docker CLI shelling vs Bollard API**: Image builds shell out to `docker build` instead of using the Bollard API that's already a dependency for container management. This creates a runtime dependency on Docker CLI being in PATH. The Bollard crate supports image builds, so there's a split between managed (containers via API) and unmanaged (builds via CLI).

- **executor.rs size**: Resolved. Split into `mod.rs`, `docker.rs`, `wave.rs`, `helpers.rs`, and `local.rs` with clear responsibility boundaries.

- **Synchronous context gathering in daemon**: Resolved. `build_step_prompt()` now runs via `spawn_blocking`, keeping the executor thread free.

- **Fork cleanup is sequential**: Resolved in CLI. `cleanup_fork_worktrees()` uses `std::thread::scope` for parallel worktree removal. Daemon fork cleanup runs tasks concurrently.

## Complexity Observations

**executor `prepare_workspace`** — The workspace preparation pipeline has 5 stages (ensure volume, shared clone, fetch, worktree, hygiene) with a mutation lock around the first three. The `prepared_key` tracking prevents redundant fetches and now includes a 5-minute TTL, so restarts re-fetch if enough time has passed.

**executor `plan_rehydration`** — Daemon restart recovery inspects every running agent, checks container state via Docker API, and classifies each as reattachable or lost. The logic is sound but the cascading state updates (agent → wave_run → wave) across multiple store calls make it hard to reason about consistency if any individual update fails. Current approach: warn and continue, which is correct for recovery.

**executor `run_fork` (WaveExecutor)** — Fork execution spawns concurrent tasks with scheduler-gated slot acquisition. On failure, it sets a `CancellationToken` and tasks check it before acquiring slots, preventing slot leaks. Tasks that are mid-execution release their slots in their cleanup path.

**lf/commands/flow.rs `run_fork` (CLI)** — The CLI fork runner creates worktrees, spawns threads, and collects results through a channel. The fail-late policy (wait for all branches, then report aggregate errors) is good, but there's no mechanism to cancel running branches if one fails catastrophically.

## Quality Observations

**Documentation**: The `docs/lfd.md` reference doc is well-structured and covers executor config, credential mounts, and Docker mode. Updated alongside code changes.

**Error messages**: Docker build failures include context about Docker CLI availability. Fork branch failures log exit codes. The credential mount allowlist produces clear rejection messages for unknown mount names.

**Test coverage**: Fork execution has unit tests for helper functions (worktree path naming, manifest serialization, direction merging, credential mount resolution) but no integration tests for the end-to-end fork execution path. The most critical new code path — parallel worktree creation, thread/task spawning, result aggregation, and cleanup on failure — is untested.

**Existing test suite**: 22 config tests, 9 wave worktree tests, 8 flow tests, 7 worktree tests, 5 agent tests. Well-written — they test behavior, use real git repos via tempdir, and avoid mock-wiring assertions.

## Extension Potential

**Executor trait**: The `AgentExecutor` trait is clean and could support additional backends (EC2, remote SSH, Kubernetes) without changing the orchestration layer.

**Fork select modes in CLI**: Adding `ForkSelect::One` to the CLI would be straightforward — pick the first branch (or prompt in interactive mode) and skip the parallel machinery. `ForkSelect::Prompt` would require a TUI picker but the direction list is already available.

**Image fingerprint as cache key**: The SHA256-based fingerprint for Dockerfile + env-setup + base image is a good foundation for more sophisticated caching — e.g., caching intermediate build layers across repos that share the same base image.

**Sequential → parallel cleanup**: Resolved. CLI uses `std::thread::scope` for parallel worktree removal. Daemon runs cleanup tasks concurrently.
