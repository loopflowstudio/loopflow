# Security Hardening

Close the vulnerability classes that affect HTTP daemons managing container-based agent execution. Informed by OWASP API Security Top 10, CIS Docker Benchmark, and patterns from Supabase, Woodpecker CI, Gitea, and Nomad.

## North Star

Every lfd deployment — local, containerized, remote — enforces auth, validates paths, and constrains agent containers. No "it's just localhost" exceptions. Security properties hold whether Concerto connects from the same machine or across the internet.

## Phases

| # | Phase | What it closes | Pre-work | Doc | Status |
|---|-------|---------------|----------|-----|--------|
| 01 | Loopback Auth | Local-process and container-to-container access without tokens | None | [01-loopback-auth.md](01-loopback-auth.md) | done |
| 02 | Path Validation | Traversal in wave IDs, worktree paths, future file APIs | None | [02-path-validation.md](02-path-validation.md) | done |
| 03 | Container Hardening | Resource limits, non-root agent user, cross-worktree isolation | None | [03-container-hardening.md](03-container-hardening.md) | done |
| 04 | API Surface Gating | Rate limiting, body size limits, error sanitization, WebSocket caps, outbound token/header leakage prevention | 01 | [04-api-surface-gating.md](04-api-surface-gating.md) | |
| 05 | Credential Hygiene | Config writes persisting secrets, log leakage, mount exfiltration risks, lightweight token separation/rotation | None | [05-credential-hygiene.md](05-credential-hygiene.md) | |
| 06 | Auth Provider Isolation | Fallthrough bypass, proxy-trust loopback, JWKS fail-open | remote/07 | [06-auth-provider-isolation.md](06-auth-provider-isolation.md) | |

Phases 03 and 05 are independent. Phase 04 depends on 01 (done — token infrastructure exists). Phase 06 depends on remote/07 (Studio auth implementation).

## Post-ship adjustments (after phases 01 and 02)

What changed after implementation:

- **Method-tier auth beat route tagging.** Phase 04 should apply limits and throttles by request tier (`read` vs `mutate`) instead of per-route tagging.
- **Centralized path guards are in place.** Path traversal controls for current surfaces are done; Phase 04 should focus on API envelope hardening, not duplicate path validation logic.
- **Biggest uncertainty moved to runtime isolation defaults.** Container limits and worktree mount isolation now carry the highest implementation risk.

## Post-ship adjustments (after phase 03)

What changed after implementation:

- **Per-wave volumes replaced subpath investigation.** The plan listed three isolation options (subpath bind mounts, workdir enforcement, accept risk). Implementation chose a fourth — full per-wave Docker volumes — which is stronger than any of the three. Trade-off is higher disk/clone cost per wave.
- **Socket proxy API allowlist is broader than planned.** The original plan listed CONTAINERS, IMAGES, VOLUMES, POST. Implementation added BUILD, PING, VERSION, INFO. Phase 04/05 should note that proxy flags may need to grow if the Docker executor gains new capabilities.
- **Default limits landed lower than the design doc.** Design doc proposed 8 GiB / 4 vCPU / 1024 PIDs. Shipped 4 GiB / 2 vCPU / 512 PIDs. The lower defaults leave more host headroom for concurrent waves and are configurable.
- **Explicit Docker network isolation was unnecessary.** Agent containers created by lfd land on the default bridge, which is already isolated from Compose-internal services. No additional network configuration needed.
- **Runtime isolation risk is now resolved.** The biggest uncertainty from Phase 03 (container limits + workspace isolation) shipped cleanly. Remaining security risk shifts to API surface (Phase 04) and credential handling (Phase 05).

Near-term sequencing stays:

1. **Phase 04** next (API envelope and leakage controls using the auth tiers already shipped)
2. **Phase 05** in parallel or immediately after 04 (credential hygiene and rotation workflow)

## Threat Model

lfd is an HTTP+WebSocket server that spawns Docker containers running AI coding agents with network access and repo volume mounts. The primary threats:

1. **Local privilege escalation** — a rogue process on the same machine (compromised dependency, browser extension, malicious repo content) reaches lfd and triggers agent execution or data access.
2. **Path traversal** — identifiers or user-supplied paths escape their expected root, reading or writing arbitrary files.
3. **Container escape** — an agent container accesses resources outside its intended scope (other worktrees, the Docker socket, the host filesystem, the database).
4. **Auth bypass** — requests reach mutation endpoints without valid credentials due to loopback trust, proxy misidentification, or provider fallthrough.
5. **Resource exhaustion** — unbounded requests, payloads, or container resource consumption cause denial of service.
6. **Credential leakage** — secrets appear in logs, error responses, persisted config, or are exfiltrated by agents via network.

## Security boundary

This roadmap is intended to make these statements true:

- If an attacker can send HTTP/WS requests to lfd (local or remote) but has no valid credentials, they cannot run mutate routes.
- If an attacker controls path-like API input, they cannot make lfd read/write outside declared roots.
- If lfd talks to another host, it does not forward internal auth headers/tokens to that host.
- If someone reads normal logs/status/error output, they do not see secrets.

This roadmap does **not** promise:

- It does not stop malware running as the same OS user from reading local session material.
- It does not fix a fully compromised host.
- It does not provide hosted-grade tenant isolation yet.

## Security invariants (must always hold)

These are non-negotiable properties. Each phase should add tests that assert them.

1. **No unauthenticated mutation** — state-changing routes never execute without valid auth.
2. **No filesystem escape** — user-controlled identifiers/paths cannot read or write outside declared roots.
3. **No cross-host auth header forwarding** — outbound redirects/host changes never receive internal auth headers.
4. **No secret in operator-visible output** — logs, status, and error payloads do not expose tokens/credentials.
5. **Fail closed on auth/trust ambiguity** — when auth source or trust context is unclear, deny by default.

## Reference Frameworks

- **OWASP API Security Top 10 (2023)**: API1 (BOLA), API2 (Broken Auth), API4 (Unrestricted Resource Consumption), API7 (SSRF), API8 (Security Misconfiguration) are directly applicable.
- **CIS Docker Benchmark**: Docker socket access, container user, capability dropping, resource limits, network isolation, image provenance.
- **Supabase architecture**: Single gateway as sole external surface, tiered key model (anon/user/service), defense-in-depth layering, socket proxy for database access.
- **Woodpecker CI / Drone**: Credential injection into pipeline containers, workspace isolation, Docker socket proxy pattern.
- **Nomad security model**: Workload identity, namespace isolation, ACL system for job orchestration.

## What's not here

- Prompt injection defenses (agent-generated content influencing future runs) — revisit at remote/09 (hosted)
- SSRF guards — revisit when lfd gains URL-fetching endpoints
- Multi-tenant authorization (per-user wave access) — revisit at remote/09
- Mobile client security — separate wave when mobile ships

## Current scope cut (agreed)

From the recent OpenClaw postmortem review, prioritize two additions now and defer broader trust-tier work:

1. **Phase 04: outbound integration hygiene**
   - Never forward `Authorization`/session headers across host changes or redirects.
   - Add regression tests proving secrets/tokens are not exposed in logs, status endpoints, or error payloads.
   - Keep forwarded-header handling fail-closed unless source IP is in explicit trusted-proxy CIDRs.
   - Scope note: Phase 04 covers proxy/header handling mechanics; Phase 06 owns auth-policy decisions that depend on proxy trust.

2. **Phase 05: lightweight credential model (not full key-tiering)**
   - Keep daemon auth token separate from user/session tokens.
   - Add one explicit token rotation path for static auth.
   - Keep credentials out of URLs/query params; headers only.

Deferred for later:
- Trusted vs untrusted execution tiers (for now: one hardened default execution profile)
- Namespace-style multi-tenant segmentation
- Full workload identity system
