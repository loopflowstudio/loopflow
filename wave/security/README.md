# Security Hardening

Close the vulnerability classes that affect HTTP daemons managing container-based agent execution. Informed by OWASP API Security Top 10, CIS Docker Benchmark, and patterns from Supabase, Woodpecker CI, Gitea, and Nomad.

## Vision

Every lfd deployment — local, containerized, remote — enforces auth, validates paths, and constrains agent containers. No "it's just localhost" exceptions. Security properties hold whether Concerto connects from the same machine or across the internet.

| # | Phase | What it closes | Pre-work | Doc | Status |
|---|-------|---------------|----------|-----|--------|
| 01 | Loopback Auth | Local-process and container-to-container access without tokens | None | [01-loopback-auth.md](01-loopback-auth.md) | done |
| 02 | Path Validation | Traversal in wave IDs, worktree paths, future file APIs | None | [02-path-validation.md](02-path-validation.md) | done |
| 03 | Container Hardening | Resource limits, non-root agent user, cross-worktree isolation | None | [03-container-hardening.md](03-container-hardening.md) | done |
| 04 | API Surface Gating | Rate limiting, body size limits, error sanitization, WebSocket caps, outbound token/header leakage prevention, proxy trust | 01 | — (shipped, see scratch/) | done |
| 05 | Credential Hygiene | Token separation/rotation, config write secret preservation, log/status redaction, transport hygiene | None | — (shipped, see [scratch/security-credential-hygiene.md](../../scratch/security-credential-hygiene.md)) | done |
| 06 | Auth Provider Isolation | JWKS fail-closed, token format pre-validation, revocation semantics | remote/04 | [06-auth-provider-isolation.md](06-auth-provider-isolation.md) | next (blocked on remote/04) |

## Goals

This roadmap is intended to make these statements true:

- If an attacker can send HTTP/WS requests to lfd (local or remote) but has no valid credentials, they cannot run mutate routes.
- If an attacker controls path-like API input, they cannot make lfd read/write outside declared roots.
- If lfd talks to another host, it does not forward internal auth headers/tokens to that host.
- If someone reads normal logs/status/error output, they do not see secrets.

### Security invariants (must always hold)

These are non-negotiable properties. Each phase should add tests that assert them.

1. **No unauthenticated mutation** — state-changing routes never execute without valid auth.
2. **No filesystem escape** — user-controlled identifiers/paths cannot read or write outside declared roots.
3. **No cross-host auth header forwarding** — outbound redirects/host changes never receive internal auth headers.
4. **No secret in operator-visible output** — logs, status, and error payloads do not expose tokens/credentials.
5. **Fail closed on auth/trust ambiguity** — when auth source or trust context is unclear, deny by default.

## Risks

Near-term sequencing now:

1. **Phase 06** next when remote/04 lands (JWKS fail-closed and token format pre-validation)
2. Keep the lightweight credential model restart-based unless operator pain proves otherwise

## Post-ship adjustments (after phase 05)

What changed after implementation:

- **Phase 05 was broader than planned at the type boundary.** `SecretString` now carries auth/registration/session secrets by default, with constant-time equality and redacted display/debug. This removed reliance on callsite logging discipline.
- **Registration exposure split cleanly by audience.** `/health` now returns only `{ enabled, registered }`; `/status` keeps detailed registration state but sanitizes `last_error`.
- **Transport hygiene moved earlier in the stack.** Query-based credentials are now rejected globally (including unauthenticated routes), and that rejection runs before trace logging middleware so sensitive query strings do not enter request traces.
- **Static token rotation is explicitly restart-based.** `lfd token rotate` generates and prints a one-time token with a restart runbook. No dual-token grace window or hot-reload was added.
- **Phase 06 scope is sharper.** Provider fallback and proxy trust concerns are already closed; remaining work is JWKS fail-closed behavior, token format pre-validation, and revocation latency documentation once remote/04 lands.

## Threat Model

lfd is an HTTP+WebSocket server that spawns Docker containers running AI coding agents with network access and repo volume mounts. The primary threats:

1. **Local privilege escalation** — a rogue process on the same machine (compromised dependency, browser extension, malicious repo content) reaches lfd and triggers agent execution or data access.
2. **Path traversal** — identifiers or user-supplied paths escape their expected root, reading or writing arbitrary files.
3. **Container escape** — an agent container accesses resources outside its intended scope (other worktrees, the Docker socket, the host filesystem, the database).
4. **Auth bypass** — requests reach mutation endpoints without valid credentials due to loopback trust, proxy misidentification, or provider fallthrough.
5. **Resource exhaustion** — unbounded requests, payloads, or container resource consumption cause denial of service.
6. **Credential leakage** — secrets appear in logs, error responses, persisted config, or are exfiltrated by agents via network.

### What might change

- Trusted vs untrusted execution tiers are currently deferred; priority could shift if threat posture changes.
- Auth-provider isolation details depend on remote/04 sequencing and may force roadmap reshuffling.
- Runtime hardening defaults may need adjustment if operator environments show unexpected constraints.
- Redaction and query-key denylists are heuristic; we may need to extend pattern coverage as real-world error formats evolve.

## Metrics

- Mutation routes are denied without valid auth (integration tests cover HTTP and WS mutate paths)
- Path traversal attempts cannot escape declared roots (regression tests for identifiers and path-like input)
- Redirects/host changes do not receive internal auth headers/tokens (outbound integration tests)
- Logs, status, and error payloads are free of credentials/secrets (redaction and leakage tests)
- Container runtime defaults enforce non-root + limits + `no-new-privileges`

- It does not stop malware running as the same OS user from reading local session material.
- It does not fix a fully compromised host.
- It does not provide hosted-grade tenant isolation yet.

## Reference Frameworks

- **OWASP API Security Top 10 (2023)**: API1 (BOLA), API2 (Broken Auth), API4 (Unrestricted Resource Consumption), API7 (SSRF), API8 (Security Misconfiguration) are directly applicable.
- **CIS Docker Benchmark**: Docker socket access, container user, capability dropping, resource limits, network isolation, image provenance.
- **Supabase architecture**: Single gateway as sole external surface, tiered key model (anon/user/service), defense-in-depth layering, socket proxy for database access.
- **Woodpecker CI / Drone**: Credential injection into pipeline containers, workspace isolation, Docker socket proxy pattern.
- **Nomad security model**: Workload identity, namespace isolation, ACL system for job orchestration.

## What's not here

- Prompt injection defenses (agent-generated content influencing future runs) — revisit at remote/06 (hosted)
- SSRF guards — revisit when lfd gains URL-fetching endpoints
- Multi-tenant authorization (per-user wave access) — revisit at remote/06
- Mobile client security — separate wave when mobile ships

## Current scope cut (agreed)

From the recent OpenClaw postmortem review, prioritize two additions now and defer broader trust-tier work:

1. **Phase 04: outbound integration hygiene** — done
   - ~~Never forward `Authorization`/session headers across host changes or redirects.~~ Shipped via `SafeHttpClient`.
   - ~~Add regression tests proving secrets/tokens are not exposed in logs, status endpoints, or error payloads.~~ Error payload sanitization shipped here; log/status redaction shipped in Phase 05.
   - ~~Keep forwarded-header handling fail-closed unless source IP is in explicit trusted-proxy CIDRs.~~ Shipped with `trusted_proxy_cidrs = []` default.
   - Scope note: Phase 04 shipped proxy/header handling mechanics and error sanitization. Phase 06 owns auth-policy decisions (JWKS, token format).

2. **Phase 05: lightweight credential model (not full key-tiering)** — done
   - ~~Keep daemon auth token separate from user/session tokens.~~ Enforced with provider-specific `SecretString` auth paths.
   - ~~Add one explicit token rotation path for static auth.~~ Shipped via `lfd token rotate` + docs runbook.
   - ~~Keep credentials out of URLs/query params; headers only.~~ Shipped via global auth-like query rejection middleware.

Deferred for later:
- Trusted vs untrusted execution tiers (for now: one hardened default execution profile)
- Namespace-style multi-tenant segmentation
- Full workload identity system
