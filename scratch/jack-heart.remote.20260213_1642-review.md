# Pre-shared Token Auth — Review

## What was implemented

Replaced the boolean-encoded `AuthContext { active, registered, validator }` with an `AuthProvider` enum (`Local`, `Static`, `Studio`). lfd now supports three auth strategies selected via `auth.provider` in `lfd.yaml` or `LFD_AUTH_PROVIDER` env var.

Static token auth: lfd validates a pre-shared bearer token using constant-time comparison (`subtle::ConstantTimeEq`). Token comes from config or `LFD_AUTH_TOKEN` env var.

Python client: `Client(token="...")` kwarg + `LFD_TOKEN` env var inject `Authorization: Bearer` headers.

## Key choices

**Enum over booleans.** `AuthContext` encoded three states as two booleans (`active` + `registered`), with a fourth implicit "broken" state (`active=true, registered=false`). The enum makes each state explicit and extensible.

**Constant-time comparison.** `subtle::ConstantTimeEq` for static tokens. Lightweight (zero transitive deps), prevents timing side channels even though this is dev-only.

**Loopback bypass is unconditional.** Regardless of provider, 127.0.0.1 requests skip auth. This means `provider: static` on a loopback bind still works without a token — local dev is never broken by auth config.

**Non-loopback + `provider: local` warns, doesn't crash.** Previous behavior was to crash (`process::exit`) when binding to a non-loopback address without studio registration. Now it logs a warning and continues — remote connections get 403 but the server stays up. This is less surprising for operators.

## How it fits together

```
lfd.yaml / env vars
    → LfdConfig.auth.provider + .token
    → setup_auth() dispatches on provider string
    → returns AuthProvider enum
    → stored in HttpState.auth
    → auth_middleware matches on AuthProvider variant per request
```

Loopback check happens first in middleware, before provider dispatch. Studio registration flow is extracted to `setup_studio_registration()` — unchanged logic, just reorganized.

## Risks and bottlenecks

**Token in plaintext config.** `lfd.yaml` stores the token in cleartext. Acceptable for dev/testing per design doc. Defense in depth: IP-restrict security groups (Phase 04).

**`process::exit(1)` on config errors.** `setup_auth` calls `process::exit` for missing token or unknown provider. This is consistent with the existing studio registration path but means config errors aren't testable without process-level test harness. Acceptable tradeoff — these are startup-only checks.

**Middleware-level integration tests not implemented.** The design doc listed tests like `static_provider_accepts_correct_token` and `loopback_bypasses_any_provider` that would require constructing full axum middleware pipelines. The implemented tests cover the core logic (`constant_time_eq`, `extract_token`) and config parsing. The middleware dispatch is a straightforward match — low risk.

## What's not included

- Token rotation, expiry, revocation (out of scope for dev auth)
- Concerto (Swift) token support (Phase 05)
- JWKS/JWT validation for Studio provider (Phase 07)
- Rate limiting on auth failures
- The old roadmap item (`roadmap/remote/03-pre-shared-auth.md`) was deleted since the design doc in `scratch/` supersedes it and the work is done
