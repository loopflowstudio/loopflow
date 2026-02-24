# Credential Hygiene (Security Phase 05)

## Problem

Credentials leak through side channels: log output, unauthenticated health endpoints, query strings, and runtime type misuse. The auth middleware correctly separates token providers (Phase 01) and sanitizes error payloads (Phase 04), but secrets can still surface in operator-visible output and the type system doesn't prevent it.

Concretely:

- `/health` is unauthenticated and returns `RegistrationState` including `machine_id`, `machine_name`, and `last_error` — the last of which can contain server URLs and error details from failed registration attempts.
- Token values are `String` throughout: `AuthProvider::Static { token: String }`, `session_token: Option<String>`, `connection_token: Arc<RwLock<Option<String>>>`. Nothing prevents `tracing::warn!(..., token = %token)` from compiling and running.
- No query-param rejection exists. Token extraction is header-only, but a request like `GET /v0/waves?token=abc123` passes without error — the token lands in access logs, proxy logs, and browser history while being silently ignored.
- Static token rotation requires manual config editing with no supported tooling or documentation.

## Approach

Four workstreams, each independently shippable:

### 1. Opaque token types (`SecretString`)

Introduce a `SecretString` newtype that wraps token values. `Display` and `Debug` emit `[REDACTED]`. Access requires explicit `.expose_secret()`.

```rust
#[derive(Clone)]
pub struct SecretString(String);

impl SecretString {
    pub fn new(s: impl Into<String>) -> Self { Self(s.into()) }
    pub fn expose_secret(&self) -> &str { &self.0 }
}

impl fmt::Debug for SecretString { ... } // => "[REDACTED]"
impl fmt::Display for SecretString { ... } // => "[REDACTED]"
impl PartialEq for SecretString { ... } // constant-time via subtle
impl Drop for SecretString { ... } // zeroize on drop
```

Migrate all token-bearing fields:

| Field | Current | After |
|-------|---------|-------|
| `AuthProvider::Static { token }` | `String` | `SecretString` |
| `HttpState.session_token` | `Option<String>` | `Option<SecretString>` |
| `RegistrationClient.connection_token` | `Arc<RwLock<Option<String>>>` | `Arc<RwLock<Option<SecretString>>>` |
| `AuthConfig.token` | `Option<String>` | `Option<SecretString>` |
| `RegisterResponse.connection_token` | `String` | `SecretString` |

After this migration, accidentally logging a token via structured tracing emits `[REDACTED]` instead of the token value. The compiler doesn't prevent the mistake, but the runtime output is safe.

### 2. Health endpoint redaction

Strip identifying and potentially-secret fields from the unauthenticated `/health` response. Move them to the authenticated `/status` endpoint.

**Before:**
```
GET /health (unauthenticated)
→ { registration: { machine_id, machine_name, last_error, ... } }
```

**After:**
```
GET /health (unauthenticated)
→ { registration: { enabled, registered } }

GET /status (authenticated)
→ { registration: { enabled, registered, machine_id, machine_name, last_error, ... } }
```

Introduce `RegistrationState::public_summary()` that returns only non-sensitive fields. Health handler uses the summary; status handler uses the full state.

Also sanitize `last_error` in the full state before returning it — apply the same `sanitize_error_message()` pipeline from Phase 04's error handling, since registration errors can contain server URLs and error details.

### 3. Query parameter rejection

Add Axum middleware that rejects requests containing auth-like query parameters with `400 Bad Request`.

Rejected parameter names: `token`, `access_token`, `auth_token`, `api_key`, `bearer`, `secret`, `password`, `credential`.

```
GET /v0/waves?token=abc → 400 {"error": {"message": "authentication credentials must not appear in query parameters"}}
GET /v0/waves → 200 (normal)
```

This is defense-in-depth. The server already doesn't read tokens from query params, but rejecting them prevents tokens from landing in access logs, proxy logs, and browser history while being silently ignored.

Applied to all routes (authenticated and unauthenticated) so that a misconfigured client gets an immediate, clear error rather than a silent auth failure.

### 4. Static token rotation

Provide a documented restart-based rotation path with a CLI command that generates a new token and prints the operational steps.

```bash
lfd token rotate
# → Generates new token
# → Prints: "New token: lfd_xxxxx"
# → Prints: "Update LFD_AUTH_TOKEN and restart lfd to activate"
```

No dual-token grace period. lfd is typically local or single-operator; restart-based rotation is sufficient. If zero-downtime rotation is needed, container orchestrators handle rolling restarts.

The command:
1. Generates a 32-byte random token, hex-encoded (same entropy as session tokens).
2. Prints the token once. Does not write it to config files (operator chooses where to store it).
3. Documents the swap procedure in a runbook section.

Add regression test: generate token, configure lfd with it, verify auth succeeds; rotate, restart, verify old token is rejected and new token is accepted.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| `secrecy` crate for opaque tokens | Battle-tested, zeroize-on-drop built in | Adds a dependency for ~30 lines of code. Implement inline; adopt `secrecy` later if needed. |
| Dual-token grace period for rotation | Zero-downtime rotation | Over-engineered for single-operator daemon. Container orchestrators solve this at the infrastructure layer. |
| Structured tracing redaction layer | Catches all log output automatically | Invasive, hard to test, false positives on non-secret strings. Opaque types are simpler and catch the problem at the source. |
| Strip query params silently instead of rejecting | More forgiving to misconfigured clients | Silent stripping masks bugs. A 400 error forces the client to fix its behavior. Follows the wave's "fail closed on ambiguity" principle. |
| Full `tracing` subscriber filter | Redact any field named "token" in structured logs | Over-broad (not all "token" fields are secrets), under-specific (secrets in unstructured strings slip through). Type-level protection is more precise. |

## Key decisions

**Opaque types over runtime redaction.** Security invariant 4 from the wave README: "No secret in operator-visible output." A newtype that can't accidentally render as plaintext upholds this at the type boundary rather than relying on every log callsite to remember redaction.

**Reject query params, don't strip them.** Security invariant 5: "Fail closed on auth/trust ambiguity." A token in a query param is ambiguous intent — reject it rather than silently ignoring it.

**Health endpoint minimalism.** The health endpoint exists for load balancers and monitoring. It doesn't need machine identity or error details. Following the principle of least privilege, expose only what probes need.

**Restart-based rotation over live reload.** The wave scope says "lightweight by design." A restart-based path with a CLI helper is simpler to implement, simpler to reason about, and simpler to test than signal-based config reloading.

## Scope

**In scope:**
- `SecretString` newtype in `lfd/` with `Debug`/`Display` redaction, constant-time equality, zeroize-on-drop
- Migrate all token fields to `SecretString`
- Health endpoint: strip `machine_id`, `machine_name`, `last_error` from unauthenticated response
- Sanitize `RegistrationState.last_error` before returning in authenticated `/status`
- Query param rejection middleware for auth-like parameter names
- `lfd token rotate` CLI subcommand (generates token, prints instructions)
- Rotation runbook documentation
- Regression tests for all four workstreams
- Cross-provider rejection tests (Local token rejected by Static provider, Static token rejected by Local provider)

**Out of scope:**
- Full IAM / key management system (wave non-goal)
- Per-wave credential scoping (tracked for multi-tenant, not this phase)
- Signal-based live config reload
- `secrecy` crate adoption (implement inline first)
- Client-side (Python/Swift) changes beyond verifying no credential mixing — client token resolution was tested in Phase 04

## Implementation order

1. **`SecretString` type + field migration** — foundational; other workstreams benefit from it
2. **Health endpoint redaction** — small, independent, immediate security improvement
3. **Query param rejection middleware** — small, independent
4. **Static token rotation CLI + tests** — depends on `SecretString` for the generated token

## Done when

```bash
# SecretString prevents accidental logging
cargo test -p loopflow secret_string

# Health endpoint doesn't leak machine identity or errors
curl http://localhost:2486/health | jq '.registration'
# → { "enabled": true, "registered": true }  (no machine_id, machine_name, last_error)

# Query params with auth tokens are rejected
curl -s 'http://localhost:2486/v0/waves?token=foo' | jq '.error.message'
# → "authentication credentials must not appear in query parameters"

# Token rotation generates valid token
lfd token rotate
# → prints new token

# Cross-provider rejection holds
cargo test -p loopflow auth_cross_provider

# Full suite green
cargo test --all && uv run pytest python/tests/
```
