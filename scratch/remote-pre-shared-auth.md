# Pre-shared Token Auth

Static bearer token auth for remote lfd connections. Phase 03 of the remote roadmap. Implemented on this branch.

## What was built

Replaced `AuthContext { active, registered, validator }` (booleans encoding a state machine) with `AuthProvider` enum (`Local`, `Static`, `Studio`). Provider selected via `auth.provider` in `lfd.yaml` or `LFD_AUTH_PROVIDER` env var.

Static token auth validates a pre-shared bearer token using constant-time comparison (`subtle::ConstantTimeEq`). Token from config (`auth.token`) or `LFD_AUTH_TOKEN` env var.

Python client: `Client(token="...")` kwarg + `LFD_TOKEN` env var inject `Authorization: Bearer` headers.

## Key decisions

**Enum over booleans.** `AuthContext` had four implicit states from two booleans. The enum makes each state explicit and extensible.

**Constant-time comparison.** `subtle::ConstantTimeEq` prevents timing side channels. Zero cost, good habit for when Studio auth ships.

**Loopback bypass is unconditional.** Regardless of provider, 127.0.0.1 requests skip auth. Local dev is never broken by auth config.

**Non-loopback + `provider: local` warns, doesn't crash.** Remote connections get 403 but the server stays up.

**Alternatives rejected:** mTLS (too heavy for dev), IP allowlist (too coarse), hashed token in config (no real benefit), token file (extra file to manage).

## Data flow

```
lfd.yaml / env vars
    -> LfdConfig.auth.provider + .token
    -> setup_auth() dispatches on provider string
    -> returns AuthProvider enum
    -> stored in HttpState.auth
    -> auth_middleware matches on AuthProvider variant per request
```

## Known limits

- Token stored in plaintext config (acceptable for dev; IP-restrict via security groups in Phase 04)
- `process::exit(1)` on config errors (startup-only, consistent with existing paths)
- Middleware-level integration tests not implemented (core logic tested via unit tests; middleware dispatch is a straightforward match)

## Not in scope

- Token rotation, expiry, revocation
- Concerto (Swift) token support (Phase 05)
- JWKS/JWT validation for Studio provider (Phase 07)
- Rate limiting on auth failures
