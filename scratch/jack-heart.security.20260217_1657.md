# Phase 03: Container Hardening (Execution Isolation by Default)

## Problem

Phases 01 and 02 closed auth and path traversal gaps, but the highest-risk path is still runtime compromise inside Docker executor containers.

Today, agent containers still run as `root`, have no CPU/memory/PID ceilings, and can escalate privileges. If one agent is compromised, blast radius is bigger than it should be.

Who benefits:
- Operators running `lfd` in `mode: container` (highest risk surface)
- Users depending on daemon stability under heavy/hostile workloads

Why now:
- Security wave sequencing already puts this next
- Phase 04 API limits are less valuable if runtime isolation is still weak

## Approach

Adopt a **secure-by-default Docker execution profile**.

**Harden every agent container runtime:**
- Run as non-root user (`agent`) in `docker/agent/Dockerfile` and container create config.
- Apply default limits in `HostConfig`:
  - memory: `8 GiB`
  - memory_swap: `8 GiB` (no extra swap)
  - cpu_quota: `400_000` (4 vCPU)
  - pids_limit: `1024`
- Apply `security_opt = ["no-new-privileges:true"]`.
- Keep limits configurable under `executor.limits.*` in `~/.lf/lfd.yaml` plus env overrides.

Implementation notes:
- Leave read-only rootfs as a follow-up flag after compatibility validation for Claude/Codex/Gemini CLI write paths.
- Keep native mode behavior unchanged (hardening targets Docker executor path).
- Workspace volumes remain repo-scoped. Per-wave directory isolation within the shared volume (worktrees) is sufficient.
- Default limits are generous — they prevent runaway containers from affecting the host, not constrain normal agent work. Tighten later based on observed usage.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Per-wave volumes | Stronger cross-wave isolation | Both waves share the same repo; directory-level separation + hygiene is sufficient. Cost (clone per wave, disk overhead) isn't proportionate. |
| Docker socket proxy | Narrows API surface for `lfd` | The threat (compromised `lfd` daemon process) is narrow, and the proxy allowlist is broad enough that it doesn't meaningfully reduce capability. Adds operational complexity. |
| Full sandbox stack (gVisor/Kata + egress controls) | Strongest isolation | Too large for this phase; delays immediate risk reduction. |

## Key decisions

- **Decision: secure defaults, explicit opt-out only via config.** Operators should not need a hardening checklist to get safe behavior.
- **Decision: generous default limits.** Prevent runaway containers, don't constrain normal work. Tighten based on data.
- **Decision: defer read-only rootfs to a focused follow-up.** We avoid shipping fragile defaults that break real agent CLIs.
- **Decision: keep repo-scoped workspace volumes.** Per-wave volumes add disk/clone overhead without proportionate security gain.

## Scope

- In scope:
  - Docker executor non-root user + resource limits + `no-new-privileges`
  - New `executor.limits` config surface and defaults
  - Regression tests for container config behavior

- Out of scope:
  - Per-wave workspace volume isolation
  - Docker socket proxy for managed compose
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
