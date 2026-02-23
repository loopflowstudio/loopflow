# 03: Container Hardening — Done

Reduce the blast radius of a compromised agent container. Containers now run as non-root with resource limits and `no-new-privileges`.

## What shipped

### Non-root agent user

`docker/agent/Dockerfile` creates a system `agent` user/group and sets `USER agent`. Docker executor uses `user: "agent"` for all agent and helper containers. Docker-gen guidance updated so generated Dockerfiles use `USER root` for build-time setup and switch back to `USER agent` for runtime.

### Resource limits and security options

All managed containers now receive:

```
memory: 8 GiB
memory_swap: 8 GiB (no swap beyond memory)
cpu_quota: 400,000 (4 vCPU)
pids_limit: 1024
security_opt: ["no-new-privileges:true"]
```

Limits are configurable via `executor.limits.*` in `~/.lf/lfd.yaml` and `LFD_EXECUTOR_LIMITS_*` env overrides. Validation rejects non-positive values. Default limits are generous — they prevent runaway containers from affecting the host, not constrain normal agent work.

### Docker-gen guidance

Updated `docker_gen.md` so generated Dockerfiles use `USER root` during setup and `USER agent` for runtime. Added "drop privileges after setup" to the guidance checklist.

## Test coverage

- `docker_host_config_applies_limits_and_no_new_privileges` — verifies HostConfig fields
- `executor_limits_accept_yaml_override` — YAML config parsing
- `invalid_executor_limits_env_override_is_rejected` — env override validation
- `executor_limits_reject_non_positive_yaml_values` — YAML validation
- Default config tests updated to assert limit fields

## Security boundary

This phase prevents:

- Agent containers from running as root or escalating privileges via setuid/setgid
- A single container from exhausting host memory, CPU, or PID space

This phase does not prevent:

- Container escape via kernel vulnerabilities (addressed by gVisor/Kata in future)
- Outbound network access from agent containers (no egress controls yet)
- A fully compromised host
- Read-only rootfs enforcement (deferred — agent CLIs may write to unexpected paths)
- Cross-wave workspace access (waves share a repo-scoped volume with directory-level worktree isolation)

## What this doesn't do

- No read-only rootfs — deferred pending compatibility validation with agent CLIs
- No per-wave workspace volume isolation — workspace volumes remain repo-scoped with per-wave worktree directories
- No Docker socket proxy — the threat (compromised lfd daemon) is narrow, and a proxy allowlist doesn't meaningfully reduce capability
- No outbound network egress controls for agent containers

## Open questions resolved

- **"Are the default limits safe?"** — Shipped 8 GiB / 4 vCPU / 1024 PIDs as generous defaults, configurable via YAML and env overrides. Tighten based on observed usage.
- **"Can Bollard mount only a worktree subpath?"** — Deferred. Repo-scoped volumes with per-wave worktree directories provide sufficient isolation. Per-wave volumes would add disk/clone overhead without proportionate security gain.
