# Phase 03 Review: Container Hardening

## What was implemented

- Agent runtime now defaults to non-root (`agent`) in both the base agent image (`docker/agent/Dockerfile`) and Docker executor container create config.
- Docker executor now applies secure default limits and hardening flags to all managed containers:
  - memory: 8 GiB
  - memory+swap: 8 GiB
  - cpu_quota: 400000 (4 vCPU)
  - pids_limit: 1024
  - `security_opt: no-new-privileges:true`
- Added `executor.limits.*` config surface with YAML + env override support and validation.
- Docs updated in `docs/lfd.md` for new limits.
- Updated docker-gen guidance so generated Dockerfiles temporarily use `USER root` for setup and return to `USER agent` for runtime.

## Key choices

- **Secure by default, configurable via explicit limits config**: hardening is always on unless operators intentionally override limits.
- **Generous default limits**: prevent runaway containers from affecting the host, don't constrain normal agent work. Tighten based on observed usage.
- **Repo-scoped workspace volumes retained**: per-wave volumes add overhead without proportionate security gain.
- **Socket proxy cut**: the threat it addresses (compromised `lfd` daemon) is narrow, and the proxy allowlist is broad enough that it doesn't meaningfully reduce capability.

## How it fits together

Config resolution (`lfd/config.rs`) now produces validated executor resource limits. The Docker executor consumes those limits when creating helper and agent containers.

## Risks and bottlenecks

- **Image/build assumptions**: custom Dockerfiles that perform privileged setup must explicitly switch to `USER root` during build and back to `USER agent`.

## What's not included

- Per-wave workspace volume isolation.
- Docker socket proxy for managed compose.
- Read-only root filesystem defaults.
- Outbound network egress controls for agent containers.
- Multi-tenant hosted authorization model changes.
- Sandbox runtime migration (gVisor/Kata).

## Validation run

- `cargo fmt --all -- --check`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test -p loopflow`
- Targeted re-check:
  - `cargo test -p loopflow lfd::config::tests::executor_limits_accept_yaml_override`
  - `cargo test -p loopflow lfd::executor::docker::tests::docker_host_config_applies_limits_and_no_new_privileges`
