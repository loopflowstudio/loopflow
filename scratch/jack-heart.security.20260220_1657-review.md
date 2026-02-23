# Phase 03 Review: Container Hardening

## What was implemented

- Agent runtime now defaults to non-root (`agent`) in both the base agent image (`docker/agent/Dockerfile`) and Docker executor container create config.
- Docker executor now applies secure default limits and hardening flags to all managed containers:
  - memory: 4 GiB
  - memory+swap: 4 GiB
  - cpu_quota: 200000 (2 vCPU)
  - pids_limit: 512
  - `security_opt: no-new-privileges:true`
- Added `executor.limits.*` config surface with YAML + env override support and validation.
- Workspace isolation changed from repo-scoped volumes to wave-scoped volumes (`repo + wave_id`) with wave cleanup now deleting the wave volume.
- Managed container-mode compose now uses `docker-socket-proxy` and `DOCKER_HOST=tcp://docker-socket-proxy:2375`, removing direct docker.sock mount from `lfd`.
- Docs updated in `docs/lfd.md` for new limits, wave-scoped volume behavior, and socket-proxy architecture.
- Updated docker-gen guidance so generated Dockerfiles temporarily use `USER root` for setup and return to `USER agent` for runtime.

## Key choices

- **Secure by default, configurable via explicit limits config**: hardening is always on unless operators intentionally override limits.
- **Per-wave volume isolation over repo-wide sharing**: prioritizes confidentiality/isolation over cache efficiency.
- **Socket proxy in managed container mode**: narrows Docker API surface exposed to the `lfd` service container.
- **Kept unrelated recovery observability behavior**: restored startup logs for orphaned fork cleanup to avoid incidental regression.

## How it fits together

Config resolution (`lfd/config.rs`) now produces validated executor resource limits. The Docker executor consumes those limits when creating helper and agent containers and uses wave-scoped volume identity for workspace mounting and cleanup. In managed container mode, generated compose routes Docker access through `docker-socket-proxy`, so `lfd` no longer receives raw socket access.

## Risks and bottlenecks

- **Cold-start and storage overhead**: per-wave volumes increase clone/fetch work and disk usage versus repo-shared workspace volumes.
- **Proxy API allowlist drift**: if future Docker executor behavior needs additional API groups, compose proxy env flags must be extended.
- **Image/build assumptions**: custom Dockerfiles that perform privileged setup must explicitly switch to `USER root` during build and back to `USER agent`.

## What's not included

- Read-only root filesystem defaults.
- Outbound network egress controls for agent containers.
- Multi-tenant hosted authorization model changes.
- Sandbox runtime migration (gVisor/Kata).

## Validation run

- `cargo fmt --all -- --check`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test -p loopflow`
- Targeted re-check after final polish:
  - `cargo test -p loopflow lfd::config::tests::executor_limits_accept_yaml_override`
  - `cargo test -p loopflow lfd::executor::docker::tests::docker_host_config_applies_limits_and_no_new_privileges`
  - `cargo test -p loopflow lfd::service::compose::tests::render_compose_uses_docker_socket_proxy`
