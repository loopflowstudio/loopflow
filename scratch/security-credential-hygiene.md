# Credential Hygiene (Security Phase 05)

## Problem

lfd already blocks obvious auth bypasses, but credentials can still leak through operator-visible paths and weak typing.

Who benefits:
- Operators running local or remote lfd get safer defaults and a clear rotation workflow.
- Loopflow users get fewer accidental auth failures from client misconfiguration.
- Security reviewers get enforceable invariants instead of log-callsite discipline.

Why now:
- Phase 04 already shipped API error sanitization and outbound header stripping.
- The remaining leaks are narrow, high-impact, and locally fixable in lfd.
- Phase 06 (provider isolation) depends on tightening token boundaries now.

## Approach

Build a fail-closed credential boundary across **types, endpoints, middleware ordering, generated config, and operator workflow**.

### 1) Opaque token type (`SecretString`) as the default credential carrier

Add `lfd::secret_string::SecretString` and migrate token-bearing fields away from `String`:

- `AuthProvider::Static { token }`
- `HttpState.session_token`
- `RegistrationClient.connection_token`
- `AuthConfig.token`
- `RegisterResponse.connection_token`

`SecretString` behavior:
- `Debug`/`Display` return `[REDACTED]`
- explicit `expose_secret()` for use sites that must read the token
- constant-time equality for secret comparison
- zeroize internal bytes on drop

Design guardrail: no blanket `From<SecretString> for String`. Secret extraction must remain explicit.

### 2) Split public vs authenticated registration status

Keep `/health` probe-safe and move sensitive registration details to `/status`:

- `/health` returns only `{ enabled, registered }`
- `/status` keeps full registration state, but sanitizes `last_error`

Implementation shape:
- add `RegistrationState::public_summary()`
- add shared redaction module used by both API errors and status redaction
- status serialization sanitizes `last_error` before response encoding

### 3) Reject auth-like query params globally before URI tracing

Add middleware that rejects requests with these query keys (case-insensitive):

`token`, `access_token`, `auth_token`, `api_key`, `bearer`, `secret`, `password`, `credential`

Behavior:
- response: `400 Bad Request`
- message: `authentication credentials must not appear in query parameters`
- applied to all routes (`/v0/*`, `/status`, `/ws`, `/health`, `/metrics`, hooks)
- ordered before request tracing so secrets in query strings are never logged

### 4) Preserve env placeholders in generated compose/config output

Lock in secret-safe rendering for generated artifacts:

- managed compose keeps `LFD_AUTH_TOKEN: "${LFD_AUTH_TOKEN:-}"`
- no interpolation of runtime token values into generated files
- add regression tests for compose output and credential env passthrough

### 5) Add explicit static token rotation command

Add `lfd token rotate`:

- generates a new 32-byte random token (hex, 64 chars)
- prints token once
- prints restart instructions: update `LFD_AUTH_TOKEN`, restart `lfd`
- does not write token into config files

Document runbook in `docs/lfd.md` with exact sequence:
1. generate token
2. update secret source (`.env`, secret manager, systemd/launchd env)
3. restart daemon
4. verify old token rejected and new token accepted

### 6) Regression coverage for provider separation and leak resistance

Add tests for:
- accidental token logging (`Debug`/`Display` redaction)
- cross-provider rejection (local token rejected by static provider and vice versa)
- `/health` redaction and `/status` sanitized `last_error`
- query-param rejection and middleware ordering
- compose placeholder preservation
- rotation flow (old token invalid after restart)

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Tracing-subscriber redaction rules | Catches some leaks at logging sink | Misses non-tracing output and string interpolation; weaker than type boundary |
| Accept query tokens but ignore them | More tolerant clients | Silent failure keeps secrets in logs/history; violates fail-closed posture |
| Dual-token grace window for rotation | Zero-downtime swaps | Adds state complexity and revocation ambiguity for little local-daemon value |
| Keep one `RegistrationState` payload everywhere | Simpler DTOs | Exposes machine identity and error details on unauthenticated endpoint |
| Use `secrecy` crate immediately | Mature ergonomics | Defers to a later cleanup; inline type is enough for this phase |

## Key decisions

- We enforce wave invariant **"No secret in operator-visible output"** at the type boundary, not by hoping every logging callsite redacts correctly.
- We enforce wave invariant **"Fail closed on auth/trust ambiguity"** by rejecting query credentials instead of silently stripping/ignoring them.
- We follow wave vision **"No 'it's just localhost' exceptions"** by applying query-param rejection to unauthenticated routes too, not only protected API routes.
- We choose restart-based rotation now: simple, testable, and sufficient for single-operator deployments.
- We keep implementation modular so Phase 06 can build on these boundaries without refactoring auth again.

## Scope

- In scope:
  - `SecretString` type and token field migration in lfd
  - `/health` registration summary + `/status` error sanitization
  - global auth-like query-param rejection middleware, pre-trace ordering
  - compose/config secret placeholder preservation regression tests
  - `lfd token rotate` command + runbook docs
  - cross-provider rejection and leak-resistance regression tests

- Out of scope:
  - full key-management or IAM system
  - per-wave credential scoping / multi-tenant policy model
  - live config reload or no-restart token hot swap
  - broad client API redesign in Python/Swift

## Done when

```bash
# token wrapper redacts debug/display and preserves constant-time compare behavior
cargo test -p loopflow secret_string

# unauthenticated health endpoint does not expose machine identity or error details
curl -s http://127.0.0.1:2486/health | jq '.registration'
# => {"enabled":true,"registered":true}

# auth-like query params are rejected before normal request handling
curl -s 'http://127.0.0.1:2486/v0/waves?token=abc' | jq '.error.message'
# => "authentication credentials must not appear in query parameters"

# rotation command emits a valid token and operational instructions
lfd token rotate

# auth provider separation remains strict
cargo test -p loopflow auth_cross_provider

# full regression baseline
cargo test --all && uv run pytest python/tests/
```
