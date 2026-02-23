# Phase 03: Container Hardening (Execution Isolation by Default)

## Problem

Phases 01 and 02 closed auth and path traversal gaps, but the highest-risk path is still runtime compromise inside Docker executor containers.

Today, agent containers still run as `root`, have no CPU/memory/PID ceilings, can see a repo-wide shared workspace volume, and (in container mode) `lfd` still has direct Docker socket access. If one agent is compromised, blast radius is bigger than it should be.

Who benefits:
- Operators running `lfd` in `mode: container` (highest risk surface)
- Teams running many waves in parallel (cross-wave isolation)
- Users depending on daemon stability under heavy/hostile workloads

Why now:
- Security wave sequencing already puts this next
- Phase 04 API limits are less valuable if runtime isolation is still weak
- The current design leaves a practical cross-wave confidentiality gap

## Approach

Adopt a **secure-by-default Docker execution profile** with three concrete changes shipped together.

1. **Harden every agent container runtime**
   - Run as non-root user (`agent`) in `docker/agent/Dockerfile` and container create config.
   - Apply default limits in `HostConfig`:
     - memory: `4 GiB`
     - memory_swap: `4 GiB` (no extra swap)
     - cpu_quota: `200_000` (2 vCPU)
     - pids_limit: `512`
   - Apply `security_opt = ["no-new-privileges:true"]`.
   - Keep limits configurable under `executor.limits.*` in `~/.lf/lfd.yaml` plus env overrides.

2. **Isolate workspace per wave (not per repo)**
   - Replace repo-wide workspace volume identity with a per-wave volume identity (`repo + wave_id`).
   - Mount only that wave volume into agent/helper containers.
   - Update cleanup to remove the wave-scoped volume when a wave is removed.
   - Preserve per-repo image build caching; isolate runtime workspace data.

3. **Harden container-mode daemon access to Docker**
   - Managed compose file adds `docker-socket-proxy` and removes direct `/var/run/docker.sock` mount from `lfd`.
   - `lfd` talks to proxy through `DOCKER_HOST=tcp://docker-socket-proxy:2375`.
   - Proxy allows only required APIs (containers/images/volumes/build) and denies broader control surfaces.

Implementation notes:
- Leave read-only rootfs as a follow-up flag after compatibility validation for Claude/Codex/Gemini CLI write paths.
- Keep native mode behavior unchanged (hardening targets Docker executor path).

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Minimal hardening (non-root + limits only) | Fastest to ship | Leaves cross-wave workspace exposure and direct socket risk intact |
| Shared repo volume with subpath mounts | Better cache efficiency | Docker subpath behavior is less portable/reliable across environments and increases operational ambiguity |
| Full sandbox stack now (gVisor/Kata + strict egress controls) | Strongest isolation | Too large for this phase; delays immediate risk reduction and complicates rollout |

## Key decisions

- **Decision: prioritize deterministic isolation over cache efficiency.** We will use per-wave volumes now, even with higher disk/fetch cost.
- **Decision: secure defaults, explicit opt-out only via config.** Operators should not need a hardening checklist to get safe behavior.
- **Decision: socket proxy is mandatory in managed container mode.** Direct Docker socket exposure is too broad for this threat model.
- **Decision: defer read-only rootfs to a focused follow-up.** We avoid shipping fragile defaults that break real agent CLIs.

Wave principles this follows:
- Security README North Star: **"Every lfd deployment … constrains agent containers."**
- Security invariant: **"Fail closed on auth/trust ambiguity."** Here, that means least-privilege runtime defaults instead of permissive container access.

## Scope

- In scope:
  - Docker executor non-root user + resource limits + `no-new-privileges`
  - New `executor.limits` config surface and defaults
  - Per-wave workspace volume isolation
  - Managed compose socket-proxy integration
  - Regression tests for container config and workspace isolation behavior

- Out of scope:
  - Read-only rootfs default
  - Outbound network egress firewalling for agent containers
  - Hosted multi-tenant authorization model (remote/09)
  - Full container sandbox runtime migration (gVisor/Kata)

## Done when

- Code-level checks pass:
  - `cargo fmt --all -- --check`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo test -p loopflow docker_`
- Runtime behavior is observable:
  - `docker inspect <agent> | jq '.[0].Config.User'` returns `"agent"`
  - `docker inspect <agent> | jq '.[0].HostConfig | {Memory, MemorySwap, CpuQuota, PidsLimit, SecurityOpt}'` shows configured limits + `no-new-privileges:true`
  - Managed compose output contains `docker-socket-proxy` and no direct docker.sock mount on `lfd`
  - A container in wave A cannot access wave B workspace data because mounts are wave-scoped
