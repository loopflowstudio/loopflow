# 03: Container Hardening — Done

Reduce the blast radius of a compromised agent container. Containers now run as non-root with resource limits, `no-new-privileges`, per-wave volume isolation, and Docker socket proxy in managed compose mode.

## What shipped

### Non-root agent user

`docker/agent/Dockerfile` creates a system `agent` user/group and sets `USER agent`. Docker executor uses `user: "agent"` for all agent and helper containers. Docker-gen guidance updated so generated Dockerfiles use `USER root` for build-time setup and switch back to `USER agent` for runtime.

### Resource limits and security options

All managed containers now receive:

```
memory: 4 GiB
memory_swap: 4 GiB (no swap beyond memory)
cpu_quota: 200,000 (2 vCPU)
pids_limit: 512
security_opt: ["no-new-privileges:true"]
```

Limits are configurable via `executor.limits.*` in `~/.lf/lfd.yaml` and `LFD_EXECUTOR_LIMITS_*` env overrides. Validation rejects non-positive values.

### Per-wave workspace volumes

Replaced repo-scoped volumes with wave-scoped volumes. Volume identity is now `hash(repo_canonical + wave_id)`, so each wave gets its own Docker volume. Wave cleanup deletes the wave volume. Repo image builds are still keyed by repo identity (shared across waves for the same repo).

This is stronger isolation than the three options originally considered (subpath bind mounts, workdir enforcement, accept risk). A container in wave A cannot see wave B's workspace data because they mount different volumes.

Trade-off: higher disk usage and cold-start cost per wave (each wave clones fresh). Acceptable given that security isolation is the priority.

### Docker socket proxy in managed compose

`render_compose_file` adds a `docker-socket-proxy` (tecnativa/docker-socket-proxy:0.3.0) service. lfd talks to the proxy via `DOCKER_HOST=tcp://docker-socket-proxy:2375`. Direct `/var/run/docker.sock` mount is removed from lfd; the proxy mounts the socket read-only.

Proxy API allowlist: CONTAINERS, IMAGES, VOLUMES, BUILD, POST, PING, VERSION, INFO. Broader operations (exec, networks, swarm, secrets, services) are denied by default.

Native mode (no Compose) continues using the Unix socket directly.

### Docker-gen guidance

Updated `docker_gen.md` so generated Dockerfiles use `USER root` during setup and `USER agent` for runtime. Added "drop privileges after setup" to the guidance checklist.

## Test coverage

- `docker_host_config_applies_limits_and_no_new_privileges` — verifies HostConfig fields
- `docker_repo_volume_identity_is_wave_scoped_and_deterministic` — verifies per-wave volume naming, same-wave determinism, cross-wave isolation
- `executor_limits_accept_yaml_override` — YAML config parsing
- `invalid_executor_limits_env_override_is_rejected` — env override validation
- `executor_limits_reject_non_positive_yaml_values` — YAML validation
- `render_compose_uses_docker_socket_proxy` — compose output contains proxy, no direct socket on lfd
- Default config tests updated to assert limit fields

## Security boundary

This phase prevents:

- Agent containers from running as root or escalating privileges via setuid/setgid
- A single container from exhausting host memory, CPU, or PID space
- A container in one wave from accessing another wave's workspace data
- The lfd container (in managed compose mode) from issuing arbitrary Docker API calls

This phase does not prevent:

- Container escape via kernel vulnerabilities (addressed by gVisor/Kata in future)
- Outbound network access from agent containers (no egress controls yet)
- A fully compromised host
- Read-only rootfs enforcement (deferred — agent CLIs may write to unexpected paths)

## What this doesn't do

- No read-only rootfs — deferred pending compatibility validation with agent CLIs
- No explicit Docker network isolation in compose — agent containers on the default bridge are already isolated from Compose-internal services (Postgres, proxy)
- No outbound network egress controls for agent containers
- No per-wave credential mount isolation (credentials still shared if configured globally)

## Open questions resolved

- **"Are 4 GB / 2 CPU / 512 PIDs safe defaults?"** — Shipped as defaults, configurable. To be tightened based on observed usage. Original design doc proposed 8 GiB / 4 vCPU / 1024 but the lower defaults are generous enough for agent workloads while leaving more host headroom for concurrent waves.
- **"Can Bollard mount only a worktree subpath?"** — Resolved by choosing per-wave volumes instead. No need for subpath mounts; full volume isolation is stronger and more portable.
