# 06: Auth Provider Isolation

Finalize remote auth-provider hardening: fail closed on JWKS issues, reject malformed bearer tokens before provider validation, and document revocation behavior. This phase hardens the `Studio` path where remote deployments carry higher exposure.

Depends on remote/07 for Slice B (JWKS + JWT validation wiring). Slice A shipped independently.

## Status snapshot

- **Slice A: shipped on this branch.**
  - `Authorization` parsing now classifies `Missing`, `Malformed`, and `Present`.
  - Malformed bearer values are denied with `401` before provider validation.
  - Bearer requirements are explicit: `Bearer <token>`, non-empty token, max 4096 bytes, no embedded whitespace/control characters.
  - Studio middleware tests confirm malformed headers never call `ConnectionValidator::validate`.
- **Slice B: pending in remote/07.**

## What still needs to ship (Slice B)

1. **JWKS fail-closed behavior**
   - Retry JWKS fetch with backoff on startup.
   - Allow daemon startup, but deny Studio auth until keys are available.
   - Keep last-known-good keyset during transient outages.
   - Deny Studio auth when cached keyset age exceeds max staleness.

2. **JWT validation rules**
   - Reject `alg:none`.
   - Validate `iss`, `aud`, `exp`, and `nbf` with ~30 second clock skew tolerance.

3. **Operator documentation**
   - Document effective revocation window based on cache + refresh cadence.
   - Document outage-mode expectations/runbook when JWKS is unavailable.

## What exists today

Phase 01 removed loopback bypass and requires credentials on all protected routes. Phase 04 shipped fail-closed trusted-proxy handling and auth-failure throttling. Phase 05 shipped credential-boundary and transport hygiene hardening. Slice A in this phase now enforces malformed-token rejection before provider validation.

Remaining gap: Studio still uses `ConnectionValidator` (service validation call + 60s cache) instead of local JWKS verification until remote/07 lands.

## What exists after this phase completes

Auth providers enforce strict boundaries end-to-end: malformed tokens are rejected before provider validation, Studio JWT verification is local and fail-closed under JWKS failures/staleness, and revocation behavior is explicit for operators.

## Security boundary for this phase

This phase prevents auth ambiguity:

- Requests are evaluated by one auth provider path, not implicit fallback behavior.
- Malformed `Authorization` values are denied before provider validation.
- Studio JWT validation failures fail closed instead of silently allowing access.

This phase does not provide:

- Fine-grained per-wave authorization (remote/09 scope).
- Full zero-trust service-to-service identity across all internal components.

## Implementation plan for Slice B

### Startup behavior

When remote/07 adds JWKS validation to the `Studio` provider:

- On startup: if JWKS fetch fails after retry/backoff, start lfd but reject Studio auth until keys are present.
- Do not block process startup entirely (health checks/diagnostics should still work).

### Refresh behavior

- On JWKS refresh failures: keep using cached keys.
- If cache age exceeds max staleness window: reject Studio auth until refresh succeeds.

### JWT validation rules

- Reject tokens with `alg:none`.
- Validate `iss`, `aud`, `exp`, `nbf` with ~30s skew.

### Operator docs

- Document revocation latency tradeoff and expected window.
- Document operational behavior during JWKS outage and recovery.

## Done when

- [x] Token format pre-validation runs before provider validation.
- [x] Malformed headers are denied with provider-specific `401` errors.
- [x] Studio malformed-header path is covered by tests proving no validator call.
- [ ] Studio validates JWTs locally from cached JWKS.
- [ ] JWKS startup/refresh failures are fail-closed with explicit stale-key policy.
- [ ] JWT claim checks (`iss`, `aud`, `exp`, `nbf`) and `alg:none` rejection are enforced.
- [ ] Operator docs cover revocation window and outage behavior.

## What this doesn't do

- Doesn't add per-wave authorization (which waves can a token access) — that's remote/09 (multi-tenant)
- Doesn't add mutual TLS between services — overkill for the current deployment topology
- Doesn't add OAuth/OIDC for third-party identity providers — Studio auth handles identity
