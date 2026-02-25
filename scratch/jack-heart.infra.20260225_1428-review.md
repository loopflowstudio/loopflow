# Review: Core Boundary Cleanup (Pass 1)

Structural deconcentration of three high-churn seams. No behavior changes.

## What was implemented

**Harness command consolidation** — The harness-specific command builders (`build_claude_command`, `build_codex_command`, `build_gemini_command`, `build_opencode_command`) stay in `engine/agent.rs` as module-level functions. `build_model_command()` dispatches via a direct `match` on the parsed harness name. A separate `apply_harness_env()` function handles model-specific environment variables (Gemini system prompt path, OpenCode config JSON). Unknown models fall back to Claude with the full model string as variant.

**Docker executor decomposition** — The 2,839-line monolithic `lfd/executor/docker.rs` was split into lifecycle modules:

| Module | Responsibility | Lines |
|--------|---------------|-------|
| `docker/mod.rs` | Orchestration, `AgentExecutor` trait impl, types | ~650 |
| `docker/image.rs` | Base/per-repo image building, volume management | ~210 |
| `docker/io.rs` | Container creation, mounts, environment, log streaming | ~410 |
| `docker/recovery.rs` | Startup rehydration, container reconciliation | ~310 |
| `docker/workspace.rs` | Git workspace prep, bidirectional host/container sync | ~660 |
| `docker/tests.rs` | Unit tests for all of the above | ~630 |

All submodules use explicit imports — no `use super::*`.

**Store capability cleanup** — `SessionStore` promoted to first-class capability. `Store` exposes `wave_state()`, `execution()`, `sessions()`, `admin()` accessors, reducing method-forwarding boilerplate.

## Key choices

1. **Functions over traits for harness dispatch.** The initial extraction used a `HarnessCommandBuilder` trait with per-model modules. This was compressed back: a trait with one method and one real impl per module added ceremony without value. A `match` on four arms is simpler and equally extensible.

2. **`use super::*` elimination in docker submodules.** Each submodule now explicitly lists its dependencies from `mod.rs`. This makes it immediately visible which types and constants each lifecycle module uses, and prevents silent dependency creep.

3. **Recovery module isolation.** The startup rehydration logic (`plan_rehydration`, `reattach_agent`, `mark_agent_lost`, `cleanup_orphaned_containers`) is the most complex subsystem in the docker executor. Isolating it in `recovery.rs` makes it reviewable independently.

## How it fits together

```
engine/agent.rs
  build_model_command() → match harness → build_{claude,codex,gemini,opencode}_command()
  launch_agent()        → build_model_command() + apply_harness_env() + spawn

lfd/executor/docker/
  mod.rs      → AgentExecutor::run/terminate, DockerExecutor construction
  image.rs    → ensure_base_image, ensure_repo_image, ensure_volume
  io.rs       → build_mounts, build_container_name, stream_logs, run_helper_command
  recovery.rs → recover_startup, plan_rehydration, reattach/mark_lost
  workspace.rs→ ensure_shared_clone, ensure_worktree, sync_to_host, prepare_workspace
```

The `AgentExecutor` trait surface and `Store` capability surface are unchanged — callers are unaffected.

## Risks and bottlenecks

- **Docker recovery complexity** is contained but not reduced. The reconciliation logic in `recovery.rs` remains high-risk if container metadata conventions drift. The mock-based recovery tests provide a safety net, but they don't run against a real Docker daemon.
- **Store deconcentration is partial.** Backend `match` dispatch still lives inside trait impls. Full backend-port extraction (`SqliteStoreBackend` / `PostgresStoreBackend`) is deferred to Pass 2.
- **STYLE.md change** adds the `use super::*` rule to the repo style guide (already in CLAUDE.md). Minor — ensures human readers of STYLE.md see it too.

## What's not included

- No behavior changes. Same commands built, same containers launched, same store operations.
- No prompt/rendering changes, trigger model changes, flow language changes, or session API changes.
- Store backend-port adapter extraction deferred to Pass 2.
- Docker-required test suite split deferred to Pass 2.

## Validation

```
cargo fmt --all -- --check     ✓
cargo clippy -- -D warnings    ✓
cargo test -p loopflow          ✓ (459 passed, 0 failed)
```
