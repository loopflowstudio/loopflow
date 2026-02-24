# Credential Hygiene (Security Phase 05)

## Scope and intent

Harden `lfd` credential handling so secrets are fail-closed at boundaries where leaks are most likely: types, HTTP surfaces, middleware ordering, generated config, and operator workflows.

## Current state on this branch

Implemented:

- Added `SecretString` as the default token carrier across auth/config/registration/http/session paths.
  - `Debug`/`Display` redact values.
  - Secret access is explicit via `expose_secret()`.
  - Equality is constant-time.
  - Internal bytes are zeroized on drop.
- Added shared operator-facing redaction (`lfd::redaction`) and applied it to API error shaping and registration status sanitization.
- Split registration exposure:
  - `/health` now returns only registration summary: `{ enabled, registered }`.
  - `/status` keeps detailed registration state but sanitizes `last_error`.
- Added global query-credential rejection middleware for auth-like keys (case-insensitive, percent-encoded variants covered), returning `400` with:
  - `authentication credentials must not appear in query parameters`
- Ensured query rejection executes before trace logging middleware so sensitive query strings do not reach request traces.
- Preserved auth env placeholders in managed compose generation (`LFD_AUTH_TOKEN: "${LFD_AUTH_TOKEN:-}"`) and added regression coverage to prevent secret interpolation.
- Added `lfd token rotate` command:
  - Generates a 32-byte random token (hex, 64 chars)
  - Prints token once and restart runbook instructions
  - Does not persist token to config files
- Documented static-token rotation workflow in `docs/lfd.md`.

## Security decisions captured

- Enforce secret safety at the type boundary, not by relying on callsite logging discipline.
- Fail closed on credential ambiguity (reject query credentials rather than ignoring them).
- Apply protections uniformly (including unauthenticated routes), not only protected API paths.
- Keep token rotation restart-based for now (simple, explicit, testable).

## Remaining follow-ups

- Redaction remains heuristic; extend pattern coverage as new token/error formats appear.
- Query denylist may need additions if new credential-like query keys are introduced.
- No dual-token grace window or hot-reload yet; rotation remains restart-based by design.

## Validation targets

```bash
cargo test -p loopflow secret_string
cargo test -p loopflow auth_cross_provider
cargo test --all
uv run pytest python/tests/
```

Manual checks:

```bash
curl -s http://127.0.0.1:2486/health | jq '.registration'
curl -s 'http://127.0.0.1:2486/v0/waves?token=abc' | jq '.error.message'
lfd token rotate
```
