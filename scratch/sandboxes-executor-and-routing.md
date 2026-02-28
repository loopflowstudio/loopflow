# Sandbox Executor and Adaptive Routing

## Problem

`DockerExecutor` manages full Docker containers through Bollard — volumes, shared git clones, tar sync, per-repo image builds, helper containers, mutation locks. This is ~1600 lines of code across four modules. Docker Sandboxes offers microVM isolation with built-in bidirectional workspace sync, collapsing the entire workspace lifecycle into three CLI commands.

Phase 1 targets Claude and Gemini (the two harnesses we run most), keeping `DockerExecutor` as fallback for machines without sandbox support and for Codex/OpenCode.

Wave goals this advances:
- "Kernel isolation boundary for agent runs"
- "Cleaner lifecycle than volume/tar-sync path"
- "Incremental adoption: Claude/Gemini first, fallback always available"

## Approach

### Why `create` + `exec`, not `run`

The Docker Sandbox CLI exposes `create`, `exec`, `stop`, `save`, `reset`, and `network` alongside `run`, `ls`, `inspect`, `rm`, `version`. Key constraints:

- `docker sandbox run` has no `-d` (detach) flag — it blocks until the command exits
- `docker sandbox exec` is a first-class command with `-e` (env vars), `-w` (workdir), `-d` (detach), `-i` (interactive)
- Host `docker exec` cannot reach inside sandbox microVMs — only `docker sandbox exec` works

The correct lifecycle is `create` + `exec` + `rm`. The `create`/`exec` split gives control over credential injection (`-e` flags on exec), workspace setup timing, and clean stdout separation.

### SandboxExecutor

Shells out to Docker Sandbox CLI via `tokio::process::Command`. Structurally closer to `LocalProcessExecutor` than `DockerExecutor` — no Bollard, no volumes, no shared clones, no tar sync.

```rust
pub struct SandboxExecutor {
    store: SharedStore,
    active: Arc<Mutex<HashMap<String, SandboxState>>>,
    agent_timeout: Duration,
}

struct SandboxState {
    sandbox_id: String,
    exec_pid: Option<u32>,  // PID of the `docker sandbox exec` process
}
```

**Lifecycle per agent run:**

1. **Create sandbox:** `docker sandbox create --name lf-{agent_id} claude {cwd}`
   - Uses `claude` template (installs Claude Code, Gemini CLI, base tools)
   - Workspace syncs bidirectionally at the same absolute path — context files written to `.lf/logs/*.context.md` on the host are immediately visible inside the sandbox
   - No volume management, no shared clones, no tar sync

2. **Exec agent command:** `docker sandbox exec -e KEY=VALUE ... -w {cwd} lf-{agent_id} -- {cmd...}`
   - Spawned as subprocess; capture PID + stdout + stderr
   - Credentials injected via `-e` flags — `provider_auth::api_key_env_names()` filtering + `provider_auth::provider_env_vars()` injection, same as `LocalProcessExecutor`
   - `store.update_agent_status(agent_id, Running, Some(pid), None)` after spawn
   - stdout/stderr piped through existing `read_stream` infrastructure
   - PID stored in active map for timeout/kill

3. **Cleanup:** `docker sandbox rm lf-{agent_id}`
   - Runs after exec completes (success or failure)
   - Also runs on terminate

**Credential injection:** Env vars only, no mounts. `LocalProcessExecutor` already works this way — it passes provider tokens from the DB and filters API key env vars by harness program. Same approach here. No `~/.claude` or `~/.config/gemini` mounts needed because tokens flow via env vars.

**Streaming:** `docker sandbox exec` writes to stdout/stderr. We spawn it via `Command` and pipe both streams through `read_stream`, identical to how `LocalProcessExecutor` handles local processes.

**Terminate:** Look up `SandboxState` from active map. Kill the exec process by PID (same as local). Then `docker sandbox stop {id}` + `docker sandbox rm {id}`.

**Timeout:** Same pattern as `LocalProcessExecutor` — `tokio::time::timeout` on the exec process wait, then kill + cleanup on expiry.

### AdaptiveContainerExecutor

Wrapper that routes by harness and sandbox availability.

```rust
pub struct AdaptiveContainerExecutor {
    sandbox: SandboxExecutor,
    docker: DockerExecutor,
    sandbox_available: Arc<OnceCell<bool>>,  // set by background probe task
    active_backend: Arc<Mutex<HashMap<String, Backend>>>,
}

#[derive(Debug, Clone, Copy)]
enum Backend {
    Sandbox,
    Docker,
}
```

**Routing logic in `run()`:**

```
if sandbox_available.get() == Some(true) && is_sandbox_harness(cmd[0]):
    try sandbox.run()
    on error:
        log warning with error
        record_backend(agent_id, Docker)
        docker.run()      // runtime fallback
else:
    record_backend(agent_id, Docker)
    docker.run()
```

If the probe hasn't completed yet (`sandbox_available.get() == None`), runs fall through to Docker. Once the probe result lands, subsequent runs use it.

`is_sandbox_harness` returns true for `"claude"` and `"gemini"`. This is a simple string match on `cmd[0]`, which is always the harness binary name (set by `build_model_command` in `engine/agent.rs`).

**Terminate:** dispatches to whichever backend the active map says owns the agent.

**Other trait methods:** All delegate based on backend. For sandbox runs: `write_to_workspace` and `remove_from_workspace` use the default trait impls (write to host filesystem; sandbox sync propagates automatically). `ensure_wave_workspace` delegates to `ensure_wave_worktree` (same as `LocalProcessExecutor` — the worktree must exist on the host for sandbox to sync it). `cleanup_ephemeral_worktree` uses the default impl. `recover_startup` calls both backends' recovery. For Docker runs, delegate everything to `DockerExecutor`.

### Startup probe

Runs once at `lfd` startup in a background task, non-blocking. Result stored in an `Arc<OnceCell<bool>>` that `AdaptiveContainerExecutor` checks on each run. Before the probe completes, Claude/Gemini runs fall through to `DockerExecutor` (same as if the probe failed).

```rust
async fn probe_sandbox_support() -> bool {
    // 1. CLI exists
    if !run_cmd("docker", &["sandbox", "version"]).success() {
        return false;
    }
    // 2. Can create sandbox
    if !run_cmd("docker", &["sandbox", "create", "--name", "lf-probe", "claude", "/tmp"]).success() {
        let _ = run_cmd("docker", &["sandbox", "rm", "lf-probe"]);
        return false;
    }
    // 3. Can exec inside sandbox
    let exec_ok = run_cmd("docker", &["sandbox", "exec", "lf-probe", "--", "true"]).success();
    // 4. Cleanup
    let _ = run_cmd("docker", &["sandbox", "rm", "lf-probe"]);
    exec_ok
}
```

Log the probe result at info level with elapsed time. If the probe fails, log the specific step that failed at warn level so operators can diagnose. The async approach avoids blocking `lfd` startup — sandbox capability becomes available whenever the probe finishes.

### Config

```rust
#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ExecutorType {
    #[default]
    Local,
    Docker,    // direct Bollard, no sandbox routing
    Sandbox,   // adaptive: sandbox where supported, docker fallback
}
```

`mode: container` in `ModeProfile::for_mode()` resolves to `ExecutorType::Sandbox`. `executor.type` remains managed by mode (explicit override is rejected). A separate opt-out flag disables sandbox routing:

```yaml
# lfd.yaml
mode: container
executor:
  sandbox: false   # disable sandbox routing, use DockerExecutor directly
```

`executor.sandbox` defaults to `true`. When `false` and mode is `container`, `ModeProfile` resolves to `ExecutorType::Docker` instead of `Sandbox`. This keeps the "mode manages executor.type" invariant intact while giving operators a narrow escape hatch.

### Recovery

`SandboxExecutor` recovery on startup:
1. `docker sandbox ls` — list all sandboxes matching `lf-*` prefix
2. For each: `docker sandbox rm` to clean up orphans
3. `store.fail_orphaned_runs()` to mark stuck DB runs as failed

No stream rehydration in phase 1. If `lfd` restarts while a sandbox agent is running, the sandbox is orphaned and cleaned up. The run is marked failed.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| `docker sandbox run` as subprocess (no `create`/`exec` split) | Simpler: one command handles sandbox + agent. But `run` couples sandbox lifecycle to agent execution — no way to inject env vars at exec time, no separation between sandbox setup and command execution. Stdout might mix sandbox setup noise with agent output. | Less control over credential injection and output streaming. The `create`/`exec` split is worth the extra command. |
| `docker sandbox run -d` + `docker exec` | Detach sandbox, exec from host. But `-d` doesn't exist on `docker sandbox run`, and `docker exec` (host Docker) can't reach inside a sandbox microVM. `docker sandbox exec` is the correct command. | Doesn't work. Host `docker exec` can't reach inside sandbox microVMs. |
| Skip `AdaptiveContainerExecutor`, just add sandbox as separate config option | Users choose sandbox vs docker explicitly. Simpler code, no routing logic. | Breaks the "no user-visible behavior change" goal. Users shouldn't need to know whether their machine supports sandboxes. The probe + adaptive routing handles this transparently. |
| Pool sandboxes per wave (reuse across agent runs) | Avoids per-run sandbox creation overhead. | Adds state management complexity (which sandboxes are idle, cleanup on stale state). Per-run is simpler and matches how `DockerExecutor` creates containers per run. Optimize later if creation latency is a problem. |

## Key decisions

**Env var injection only, no credential mounts.** `LocalProcessExecutor` already works without mounts. Sandbox agents get tokens via env vars. This avoids the mount complexity in `DockerExecutor` and sidesteps questions about host home directory visibility in microVMs.

**`claude` template for both Claude and Gemini.** The `claude` template includes the base tools we need. Gemini CLI should be installable via `docker sandbox exec` if not pre-installed, or we use `--template` with our own image. Start with `claude` template and verify Gemini works inside it.

**One sandbox per agent run.** Simple, stateless, matches `DockerExecutor`'s container-per-run model. No pooling, no reuse, no stale state.

**`SandboxExecutor` models after `LocalProcessExecutor`.** Both spawn a subprocess, capture PID + streams, enforce timeout, kill on terminate. The only difference is the subprocess is `docker sandbox exec` instead of the agent binary directly. This means `SandboxExecutor` is ~150-200 lines, not ~1600 like `DockerExecutor`.

## Scope

**In scope:**
- `SandboxExecutor` struct implementing `AgentExecutor` trait
- `AdaptiveContainerExecutor` wrapper with harness routing
- Startup probe with cached result
- Runtime fallback to `DockerExecutor` on sandbox failure
- `ExecutorType::Sandbox` config variant
- `mode: container` defaults to `Sandbox` (adaptive)
- Startup recovery: orphan cleanup via `docker sandbox ls` + `rm`
- Tests: unit tests for routing logic, probe mock, integration test for sandbox lifecycle

**Out of scope:**
- Codex/OpenCode sandbox routing (phase 2+)
- DinD support (phase 2+)
- Stream reattach on restart (phase 2+)
- Bollard removal (phase 3)
- Custom template strategy (phase 2+)
- Credential proxy (phase 2+)

## File plan

| File | Change |
|------|--------|
| `rust/loopflow/src/lfd/executor/mod.rs` | Add `pub(crate) mod sandbox;` |
| `rust/loopflow/src/lfd/executor/sandbox.rs` | New: `SandboxExecutor` (~200 lines) |
| `rust/loopflow/src/lfd/executor/adaptive.rs` | New: `AdaptiveContainerExecutor` (~200 lines) |
| `rust/loopflow/src/lfd/config.rs` | Add `Sandbox` variant to `ExecutorType`, add `executor.sandbox` bool (default `true`), update `ModeProfile::for_mode()` to resolve `Sandbox` vs `Docker` based on the flag, update `explicit_executor_type_in_yaml_is_rejected` test |
| `rust/loopflow/src/lfd/executor/wave/mod.rs` | Update executor construction to build `AdaptiveContainerExecutor` when type is `Sandbox` |

## Done when

- `cargo test --all` passes with new sandbox executor tests
- `cargo clippy -- -D warnings` clean
- `SandboxExecutor` implements full `AgentExecutor` trait via `docker sandbox create/exec/rm`
- `AdaptiveContainerExecutor` routes claude/gemini to sandbox, others to docker
- Startup probe runs async (non-blocking), gates sandbox path, logs result with elapsed time
- Runtime fallback to Docker on sandbox failure with logged reason
- `mode: container` resolves to `ExecutorType::Sandbox`
- `executor.sandbox: false` opts out to `ExecutorType::Docker` (no sandbox, no probe)
- Orphan cleanup on startup via `docker sandbox ls` + `rm`
