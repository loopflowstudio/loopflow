# Security Hardening

Close the vulnerability classes that affect HTTP daemons managing container-based agent execution. Informed by OWASP API Security Top 10, CIS Docker Benchmark, and patterns from Supabase, Woodpecker CI, Gitea, and Nomad.

## Vision

Every lfd deployment — local, containerized, remote — enforces auth, validates paths, and constrains agent containers. No "it's just localhost" exceptions. Security properties hold whether Concerto connects from the same machine or across the internet.

### Scope boundaries (not here)

- Prompt injection defenses (agent-generated content influencing future runs) — revisit at remote/09 (hosted)
- SSRF guards — revisit when lfd gains URL-fetching endpoints
- Multi-tenant authorization (per-user wave access) — revisit at remote/09
- Mobile client security — separate wave when mobile ships
- Same-user local process compromise reading local session material
- Fully compromised host recovery
- Hosted-grade tenant isolation (not yet)

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

### Threat Model

lfd is an HTTP+WebSocket server that spawns Docker containers running AI coding agents with network access and repo volume mounts. The primary threats:

1. **Local privilege escalation** — a rogue process on the same machine (compromised dependency, browser extension, malicious repo content) reaches lfd and triggers agent execution or data access.
2. **Path traversal** — identifiers or user-supplied paths escape their expected root, reading or writing arbitrary files.
3. **Container escape** — an agent container accesses resources outside its intended scope (other worktrees, the Docker socket, the host filesystem, the database).
4. **Auth bypass** — requests reach mutation endpoints without valid credentials due to loopback trust, proxy misidentification, or provider fallthrough.
5. **Resource exhaustion** — unbounded requests, payloads, or container resource consumption cause denial of service.
6. **Credential leakage** — secrets appear in logs, error responses, persisted config, or are exfiltrated by agents via network.

### What might change

- Trusted vs untrusted execution tiers are currently deferred; priority could shift if threat posture changes.
- Auth-provider isolation details depend on remote/07 sequencing and may force roadmap reshuffling.
- Runtime hardening defaults may need adjustment if operator environments show unexpected constraints.

## Metrics

- Mutation routes are denied without valid auth (integration tests cover HTTP and WS mutate paths)
- Path traversal attempts cannot escape declared roots (regression tests for identifiers and path-like input)
- Redirects/host changes do not receive internal auth headers/tokens (outbound integration tests)
- Logs, status, and error payloads are free of credentials/secrets (redaction and leakage tests)
- Container runtime defaults enforce non-root + limits + `no-new-privileges`

