# Security credential hygiene review

## What was implemented

- Added `SecretString` as an opaque credential type and migrated token-bearing lfd paths to use it (`auth`, `config`, `registration`, HTTP state, session token handling).
- Added centralized operator-facing redaction (`lfd::redaction`) and wired it into API error responses and sanitized registration status payloads.
- Split registration exposure across endpoints:
  - `/health` now returns only `{ enabled, registered }` registration summary.
  - `/status` remains detailed but sanitizes `last_error`.
- Added global query-credential rejection middleware for auth-like query keys (including case-insensitive and percent-encoded variants) and ensured it runs before trace logging middleware.
- Preserved auth token placeholders in managed compose generation (`LFD_AUTH_TOKEN: "${LFD_AUTH_TOKEN:-}"`) with regression tests to prevent secret interpolation.
- Added `lfd token rotate` command to generate a 32-byte hex token and print a one-time rotation runbook.
- Updated `docs/lfd.md` with a concrete static-token rotation workflow.

## Key choices

- **Type-level boundary for secrets**: `SecretString` redacts `Debug`/`Display`, requires explicit extraction (`expose_secret`), supports constant-time comparison, and zeroizes on drop.
- **Fail-closed query handling**: requests containing auth-like query params are rejected with `400` instead of ignored/stripped.
- **Logging safety by middleware order**: query rejection executes before HTTP tracing so sensitive query strings are blocked before trace capture.
- **Operational simplicity for rotation**: token rotation is restart-based (`lfd token rotate` + update env + restart), avoiding dual-token complexity.

## How it fits together

Auth and registration handling now flow through a stricter secret boundary: credentials are stored/compared via `SecretString`, operator-visible responses are sanitized through shared redaction, and unsafe inbound credential patterns (query params) are rejected at the edge before observability middleware. The compose and CLI updates close operational leak paths by keeping env placeholders in generated artifacts and giving operators an explicit rotation command/runbook.

## Risks and bottlenecks

- Redaction is heuristic (token/path pattern matching). It materially reduces leakage risk, but malformed/new secret formats could still evade patterns.
- Query-key rejection depends on key classification. New credential-like key names would need to be added to the denylist.
- `xcodebuild test -scheme Concerto` currently fails in this environment on `ConcertoUITests.ScreenshotPipelineTests/testCapture` (window-not-found timing issue); this appears unrelated to the Rust/security diff.

## What's not included

- No hot-reload or dual-token grace period for zero-downtime token rotation.
- No broader IAM/key-management redesign.
- No client API redesign beyond endpoint payload hardening and error sanitization.
