# Phase 06 Auth Provider Isolation — Current State and Remaining Work

## Current state

Phase 06 was split into two slices to unblock security hardening:

- **Slice A: shipped on this branch.**
  - `Authorization` parsing now classifies `Missing`, `Malformed`, and `Present`.
  - Malformed bearer values are denied with `401` **before** provider validation.
  - Bearer requirements are explicit: `Bearer <token>`, non-empty token, max 4096 bytes, no embedded whitespace/control characters.
  - Studio-specific tests confirm malformed headers do not call `ConnectionValidator::validate`.
- **Slice B: still pending in `remote/07`.**

This preserves wave goals from `wave/security/README.md`:
- No unauthenticated mutation.
- Fail closed on auth/trust ambiguity.

## Remaining work (remote/07)

Complete Studio trust hardening with fail-closed JWKS behavior:

1. **Startup behavior**
   - Retry JWKS fetch with backoff.
   - Allow daemon startup, but deny Studio auth until keys are available.

2. **Refresh behavior**
   - Keep last-known-good keyset during transient outages.
   - Deny Studio auth when keyset age exceeds the defined max staleness window.

3. **JWT validation rules**
   - Reject `alg:none`.
   - Validate `iss`, `aud`, `exp`, and `nbf` (30s skew).

4. **Operator documentation**
   - Document effective revocation window based on JWKS cache + refresh cadence.
   - Add outage-mode expectations/runbook for JWKS unavailability.

## Guardrails

- No provider fallback: only the configured auth provider is used.
- Keep malformed/missing/invalid failures distinct for triage.
- Keep validation coverage across Local, Static, and Studio paths.

## Out of scope for this phase file

- Concerto sign-in UX/device selection changes.
- CLI device-code auth flow.
- Auth-server schema or contract work outside `remote/07`.
