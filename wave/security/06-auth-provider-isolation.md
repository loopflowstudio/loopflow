# 06: Auth Provider Isolation

Finalize remote auth-provider hardening: fail closed on JWKS issues, reject malformed bearer tokens before provider validation, and document revocation behavior. This phase hardens the `Studio` path where remote deployments carry higher exposure.

Depends on remote/07 (Studio auth client wiring) since it hardens the `Studio` auth provider path.

## What exists today

Phase 01 shipped a clean auth middleware: all protected routes require a valid bearer token regardless of provider or source. No loopback bypass. The middleware matches on `AuthProvider` enum with each variant handling its own validation. No fallthrough between providers.

Phase 04 shipped proxy-aware source IP handling: `trusted_proxy_cidrs` defaults to `[]` (fail-closed), `X-Forwarded-*` honored only when peer IP is in configured trusted CIDRs, malformed/ambiguous forwarded headers fall back to peer IP. Also shipped auth-failure rate throttling per source IP.

Phase 05 shipped credential-boundary hardening: `SecretString` now carries auth/session/registration tokens, static/local token checks use constant-time comparison through provider-specific paths, auth-like query params are globally rejected before trace logging, and registration status exposure is split (`/health` summary, `/status` sanitized detail).

Remaining gaps:

**Studio validator caching**: `ConnectionValidator` caches validation results for 60 seconds. If a token is revoked server-side, it remains valid in lfd for up to 60 seconds. Acceptable for the threat model, but should be documented.

**No JWKS validation yet**: The `Studio` provider currently validates connection tokens via the registration service (HTTP call to `auth.loopflow.studio`). Phase remote/07 plans to add local JWKS validation. The failure mode matters: if JWKS fetch fails on startup, does lfd reject all Studio auth (fail closed) or accept all (fail open)?

**Token format not pre-validated**: The `extract_token` function strips `Bearer ` prefix and trims whitespace, but does not enforce provider-appropriate token bounds/shape. A malformed header token still reaches provider validation logic.

## What exists after this

Auth providers enforce strict boundaries end-to-end: JWKS validation fails closed for `Studio`, token format is pre-validated before provider validation, and revocation latency is explicitly documented. (Loopback bypass, proxy trust, and credential transport hygiene are already shipped.)

## Security boundary for this phase

This phase prevents auth ambiguity:

- Requests are evaluated by one auth provider path, not implicit fallback behavior.
- `Static`/`Studio` modes do not inherit localhost bypass behavior meant for `Local`.
- JWT validation errors fail closed instead of silently allowing access.

This phase does not provide:

- Fine-grained per-wave authorization (remote/09 scope).
- Full zero-trust service-to-service identity across all internal components.

## Implementation

### Loopback bypass — already removed

Phase 01 removed the loopback read bypass entirely. All providers require credentials on all protected routes. No further work needed here.

### JWKS fail-closed

When remote/07 adds JWKS validation to the `Studio` provider:

- On startup: if JWKS fetch fails after 3 retries with backoff, lfd should start but log a warning and reject all `Studio` auth requests until JWKS is available. Not block startup entirely (that would prevent health checks and diagnostics).
- On refresh: if JWKS refresh fails, continue using the cached keyset. Log a warning. Only reject tokens if the cached keyset has expired (e.g., older than 24 hours with no successful refresh).
- Never accept `alg: none` in JWT headers.
- Validate `iss`, `aud`, `exp`, `nbf` claims. Reject tokens with `exp` in the past (no grace period beyond clock skew tolerance of ~30 seconds).

### Token format pre-validation

Query-based credentials are already rejected globally in Phase 05. This step only hardens header token parsing before provider validation.

Before passing tokens to the provider, validate basic format:

```rust
fn extract_token(headers: &HeaderMap) -> Option<&str> {
    let auth = headers.get("authorization")?.to_str().ok()?;
    let token = auth
        .strip_prefix("Bearer ")
        .or_else(|| auth.strip_prefix("bearer "))?;
    let token = token.trim();

    // Reject obviously malformed tokens
    if token.is_empty() || token.len() > 4096 || token.contains('\0') {
        return None;
    }

    Some(token)
}
```

The 4096-byte cap prevents memory abuse from pathologically large Authorization headers. Null byte rejection prevents injection.

### Proxy-aware source IP — done (Phase 04)

Shipped in Phase 04 as part of HTTP security hardening. `trusted_proxy_cidrs` defaults to `[]` (fail-closed). `X-Forwarded-*` honored only when peer IP is in configured CIDRs. Malformed/ambiguous forwarded headers fall back to peer IP. Auth-failure throttle keys on resolved source IP. No further work needed.

### Document token revocation latency

The 60-second cache in `ConnectionValidator` means revoked Studio tokens remain valid for up to 60 seconds. Document this in the remote/07 design doc as an accepted tradeoff (vs. calling the auth service on every request).

## What this doesn't do

- Doesn't add per-wave authorization (which waves can a token access) — that's remote/09 (multi-tenant)
- Doesn't add mutual TLS between services — overkill for the current deployment topology
- Doesn't add OAuth/OIDC for third-party identity providers — Studio auth handles identity
