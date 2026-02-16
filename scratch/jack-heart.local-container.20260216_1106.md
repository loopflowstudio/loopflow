# Container lfd as local option

`lfd install` works in both native and container modes. A `mode` field in `lfd.yaml` selects a strict operational profile. On this machine, container is the default — but the global default for new users stays native.

## Context

Today lfd runs as a native binary managed by launchd (macOS) or systemd (Linux). A `docker/docker-compose.yml` exists for containerized deployment but it's a separate path — users manage it manually with `docker compose` commands.

The goal: make `lfd install/start/stop/status` work seamlessly for container mode too. One CLI, two backends.

> "the container lfd is the local default ON THIS COMPUTER"
> "don't want to change the base default for new customers"
> "mode is a way to set multiple configs at once"
> "rather than configs override modes"

## Accepted decisions

1. **`mode` is strict, not a soft defaults layer.**
   - Profile-owned fields are fixed by mode:
     - runtime backend
     - storage
     - executor type
   - User config and env vars may override only non-identity fields (image, credentials mounts, auth, port).

2. **Internal model is two-axis.**
   - `service_manager`: launchd/systemd
   - `runtime_backend`: native/compose
   - `mode` maps to both.

3. **Conflict handling is fail-closed by default.**
   - `install` and `start` error if the other mode is active.
   - `--force` tears down conflicting install before proceeding.

4. **Compose file is a managed artifact.**
   - Always regenerated at install time from embedded template.
   - Optional user override file is supported separately.

5. **Status output is stable and structured.**
   - Always reports manager state, backend state, service health, and remediation hints.

## Config model

### Mode as a profile

`mode` in `lfd.yaml` selects a profile. Profile-owned fields are not user-overridable.

```yaml
# ~/.lf/lfd.yaml
mode: container

# optional overrides:
executor:
  image: my-org/agent:custom
```

Resolution order:
```
global defaults → mode profile expansion → allowed yaml overrides → allowed env overrides
```

### Mode definitions

| Setting | `native` (default) | `container` |
|---------|-------------------|-------------|
| `service_manager` | `launchd` (macOS) / `systemd` (Linux) | `launchd` / `systemd` (same OS default) |
| `runtime_backend` | `native` | `compose` |
| `storage` | `sqlite` | `postgres` |
| `executor.type` | `local` | `docker` |

### Data structures

```rust
#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    #[default]
    Native,
    Container,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum ServiceManager {
    #[default]
    Launchd,
    Systemd,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeBackend {
    #[default]
    Native,
    Compose,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum StorageType {
    #[default]
    Sqlite,
    Postgres,
}
```

### Resolution logic

`LfdConfig` gains a `mode` field and a `resolve()` method that produces a `ResolvedConfig`. The resolved config is what the rest of the codebase uses.

```rust
impl LfdConfig {
    /// Resolve mode profile, then apply allowed overrides.
    pub fn resolve(&self) -> ResolvedConfig {
        let base = match self.mode {
            Mode::Native => ModeDefaults::native(),
            Mode::Container => ModeDefaults::container(),
        };

        ResolvedConfig {
            service_manager: base.service_manager,
            runtime_backend: base.runtime_backend,
            storage: base.storage,
            executor_type: base.executor_type,
            executor_image: self.executor.image.clone(), // allowed override
            // ...
        }
    }
}
```

Invalid overrides on profile-owned fields fail config validation with a targeted error message and fix hint.

## Service management

### `lfd install` — works in both modes

Reads `lfd.yaml`, resolves mode/profile, dispatches by `(service_manager, runtime_backend)`:

**Native mode (existing behavior):**
- Writes launchd plist pointing at `lfd serve`
- `launchctl load`

**Container mode:**
1. Check Docker CLI is available and socket exists (runtime-agnostic — works with Docker Desktop, OrbStack, Colima)
2. Generate `~/.lf/docker-compose.yml` from the template, with:
   - Credential mounts based on config (claude, codex, ssh, etc.)
   - Port from config
   - API keys from env
3. Pull images (`docker compose pull`)
4. Write service unit (launchd plist on macOS, systemd user unit on Linux) that runs `docker compose -f ~/.lf/docker-compose.yml up` (not `-d` — service manager owns lifecycle)
5. Load/enable unit via service manager

Service manager (launchd/systemd) manages lifecycle in both modes. This handles:
- Boot persistence (RunAtLoad)
- Crash recovery (KeepAlive)
- Docker not ready yet after reboot (ThrottleInterval retries)

Container services also have `restart: unless-stopped` as belt-and-suspenders.

### `lfd start/stop/status` — same dispatch

```rust
// service/mod.rs — dispatch based on resolved service manager + runtime backend
pub fn install() -> Result<(), Box<dyn std::error::Error>> {
    let cfg = load_resolved_config()?;
    match (cfg.service_manager, cfg.runtime_backend) {
        (ServiceManager::Launchd, RuntimeBackend::Native) => macos::install_native(),
        (ServiceManager::Launchd, RuntimeBackend::Compose) => macos::install_compose(),
        (ServiceManager::Systemd, RuntimeBackend::Native) => linux::install_native(),
        (ServiceManager::Systemd, RuntimeBackend::Compose) => linux::install_compose(),
    }
}
```

**Status in container mode** checks both launchd and container health:
```
$ lfd status
lfd is running (container mode)
  gateway:  healthy (up 3h)
  postgres: healthy (up 3h)
```

### Compose file generation

The compose file at `~/.lf/docker-compose.yml` is generated, not copied. This lets us:
- Bake in credential mounts from config
- Set the correct image tag
- Configure ports
- Include env vars for API keys

Template lives in the lfd binary (embedded). `lfd install` renders it with the user's config.

## Dual-run protection

Both modes bind `127.0.0.1:2486`. Running both simultaneously = port conflict.

### On install

Before installing in either mode, check for the other:
- If conflict detected, fail with clear instructions.
- `--force` explicitly tears down the conflicting install and continues.

### On start

Before starting, probe port 2486:
- If something's listening, try to identify it:
  - `launchctl list com.loopflow.lfd` — is native lfd loaded?
  - `docker compose -f ~/.lf/docker-compose.yml ps` — is container stack running?
- Print a clear message: "Port 2486 is in use by [native lfd / container stack / unknown process]. Stop it first with `lfd stop` or change the port."

### On uninstall

`lfd uninstall` in container mode:
1. `docker compose down`
2. Remove launchd/systemd service unit
3. Remove generated `~/.lf/docker-compose.yml` (managed file)
4. **Does not** remove postgres data volume (user must explicitly `docker volume rm`)

## Container runtime detection

Be runtime-agnostic. Don't assume Docker Desktop.

```rust
fn detect_container_runtime() -> Result<ContainerRuntime, Error> {
    // 1. Check `docker` CLI exists on PATH
    // 2. Check socket exists (try /var/run/docker.sock, then $DOCKER_HOST)
    // 3. Run `docker info` to verify connectivity
    // 4. Return runtime info (name, version, socket path)
}
```

This works with Docker Desktop, OrbStack, Colima, Rancher Desktop — anything that exposes the standard Docker CLI and socket.

On install, if Docker isn't available, print:
```
Container mode requires Docker. Install Docker Desktop, OrbStack, or Colima, then run `lfd install` again.
```

## What changes

### Files modified

| File | Change |
|------|--------|
| `rust/loopflow/src/lfd/config.rs` | Add `Mode`, `ServiceManager`, `RuntimeBackend`, `StorageType`. Add strict profile validation + `resolve()`. |
| `rust/loopflow/src/lfd/service/mod.rs` | Dispatch on `(service_manager, runtime_backend)` |
| `rust/loopflow/src/lfd/service/macos.rs` | Manager adapter for launchd native/compose install + lifecycle |
| `rust/loopflow/src/lfd/service/linux.rs` | Manager adapter for systemd native/compose install + lifecycle |
| `rust/loopflow/src/bin/lfd.rs` | Pass resolved config to service commands |

### Files added

| File | Purpose |
|------|---------|
| `rust/loopflow/src/lfd/service/compose.rs` | install/uninstall/start/stop/status for compose mode |
| Embedded compose template | Template rendered with user config during install |

### Files not changed

- `docker/docker-compose.yml` — stays as the development/CI compose file
- `python/loopflow/` — lfq talks to lfd over HTTP regardless of how lfd runs
- `docker/lfd/Dockerfile` — the container image is the same

## Constraints

- **Mode default must stay `native`** for new users. Container mode is opt-in via `mode: container` in `lfd.yaml`.
- **lfq doesn't change.** It talks HTTP to `127.0.0.1:2486` regardless. Container vs native is invisible to the client.
- **No Docker dependency for native mode.** Container runtime detection only happens when `runtime_backend: compose` is resolved.
- **Postgres data survives `lfd stop` and `lfd uninstall`.** Explicit volume removal is a separate action.

## Done when

1. `mode: container` in `~/.lf/lfd.yaml` causes `lfd install` to set up compose via launchd
2. `lfd start/stop/status` work correctly in container mode
3. `lfd install` and `lfd start` fail with actionable error if the other mode is already installed (unless `--force`)
4. `lfd status` shows container health when in compose mode
5. Tests: strict profile validation (YAML/env), dispatch by manager/backend, dual-run detection
6. Works with OrbStack (not just Docker Desktop)

```bash
# Verify: set mode, install, check status
echo 'mode: container' > ~/.lf/lfd.yaml
lfd install
lfd status
# Expected: "lfd is running (container mode)" with healthy services

# Verify: dual-run protection
echo 'mode: native' > ~/.lf/lfd.yaml
lfd install
# Expected: hard error about existing container installation with exact teardown/--force guidance
```

## Implementation notes

- Keep code default mode as `native`; do not hardcode local machine preference in defaults.
- Keep profile-owned env vars invalid in strict mode:
  - reject `LFD_STORAGE`, `LFD_EXECUTOR_TYPE`, and runtime backend override envs if set explicitly.
  - allow non-identity envs (tokens, auth token, image tag if policy allows).
- Use multi-signal conflict detection before install/start:
  - service manager state (launchctl/systemctl)
  - compose stack state (`docker compose ps`)
  - port probe on configured bind address
- Define a stable status contract:
  - `manager: running|stopped|missing`
  - `backend: native|compose`
  - per-service health (compose mode)
  - remediation line when unhealthy
