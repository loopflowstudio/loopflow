# 06: Auth Provider Isolation

Ensure auth providers are mutually exclusive per-request, proxy topologies don't create loopback bypasses, and JWT validation fails closed. This phase hardens the auth system for remote deployments where the stakes are higher.

Depends on remote/07 (Studio auth client wiring) since it hardens the `Studio` auth provider path.

## What exists today

Phase 01 shipped a clean auth middleware: all protected routes require a valid bearer token regardless of provider or source. No loopback bypass. The middleware matches on `AuthProvider` enum with each variant handling its own validation. No fallthrough between providers.

Other gaps:

**Studio validator caching**: `ConnectionValidator` caches validation results for 60 seconds. If a token is revoked server-side, it remains valid in lfd for up to 60 seconds. Acceptable for the threat model, but should be documented.

**No JWKS validation yet**: The `Studio` provider currently validates connection tokens via the registration service (HTTP call to `auth.loopflow.studio`). Phase remote/07 plans to add local JWKS validation. The failure mode matters: if JWKS fetch fails on startup, does lfd reject all Studio auth (fail closed) or accept all (fail open)?

**Token format not validated**: The `extract_token` function strips `Bearer ` prefix and trims whitespace, but doesn't validate token format (length, character set). A malformed token still reaches the provider's validation logic.

## What exists after this

Auth providers enforce strict boundaries: no loopback bypass for `Static` or `Studio` providers, JWKS validation fails closed, and proxy-aware source IP handling is explicit.

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

### Proxy-aware source IP (future consideration)

If lfd needs to support trusted proxy headers (`X-Forwarded-For`, `X-Real-IP`), add an explicit config option:

```yaml
auth:
  provider: static
  trust_proxy_headers: true
  trusted_proxies:
    - "172.18.0.0/16"  # Docker network
```

Do not trust proxy headers by default. Only resolve source IP from headers when `trust_proxy_headers` is true and the direct connection comes from a trusted proxy CIDR. This prevents an attacker from spoofing `X-Forwarded-For: 127.0.0.1` to trigger loopback bypass.

This is not needed today (Caddy doesn't set forwarded headers in the default config, and lfd doesn't read them). Document as a constraint: "lfd uses direct connection source IP for auth decisions, not forwarded headers."

### Document token revocation latency

The 60-second cache in `ConnectionValidator` means revoked Studio tokens remain valid for up to 60 seconds. Document this in the remote/07 design doc as an accepted tradeoff (vs. calling the auth service on every request).

## What this doesn't do

- Doesn't add per-wave authorization (which waves can a token access) — that's remote/09 (multi-tenant)
- Doesn't add mutual TLS between services — overkill for the current deployment topology
- Doesn't add OAuth/OIDC for third-party identity providers — Studio auth handles identity
