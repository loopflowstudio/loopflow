# Phase 06A: Auth Provider Isolation (Unblock Plan)

## Problem

Phase 06 is the only remaining security wave item, but it is blocked on remote/07. Waiting leaves a known auth-hardening gap open: `extract_token` accepts malformed bearer values and forwards them into provider validation.

This design targets the wave vision that auth must hold in every deployment mode, and advances two explicit goals from `wave/security/README.md`:

- "No unauthenticated mutation"
- "Fail closed on auth/trust ambiguity"

Who benefits now:
- Remote operators get stricter rejection of malformed auth input before expensive validation paths.
- Security reviewers get a clear staged plan instead of an all-or-nothing dependency on remote/07.

## Approach

Ship **Phase 06 in two slices** instead of waiting for one large dependent change.

### Slice A (ship now): token format pre-validation in auth middleware

1. Replace `extract_token(&HeaderMap) -> Option<&str>` with a parse result enum:
   - `Missing`
   - `Malformed`
   - `Present(&str)`
2. Enforce provider-agnostic bearer constraints before any provider call:
   - Must be `Bearer <token>` (case-insensitive scheme).
   - Token must be non-empty after trim.
   - Token length <= 4096 bytes.
   - Reject control characters (including `\0`) and embedded whitespace.
3. Middleware behavior:
   - `Missing` keeps current missing-token behavior.
   - `Malformed` returns 401 immediately and does not call provider validators.
   - `Present` continues through existing provider-specific validation.
4. Add focused tests in `auth.rs` for parser behavior and middleware short-circuit behavior.

### Slice B (activate when remote/07 lands): JWKS fail-closed and revocation semantics

Define and implement Studio JWT validation lifecycle contract:
- Startup: retry JWKS fetch with backoff; if unavailable, daemon still starts but Studio auth remains denied (fail-closed).
- Refresh: reuse last known-good keyset on transient failures; deny Studio auth when keyset exceeds max staleness.
- JWT checks: reject `alg:none`; require valid `iss`, `aud`, `exp`, `nbf` (30s skew).
- Document effective revocation window from cache/refresh behavior.

### Research used

This follows common hardening patterns already reflected in our security docs and architecture:
- OWASP API auth guidance: reject ambiguous auth input early.
- OIDC/JWKS operational pattern: cached keys with bounded staleness and fail-closed validation.
- Existing loopflow architecture: strict provider isolation (`AuthProvider` enum) with no fallback path.

### Wild success / wild failure check

- Wild success: malformed-token traffic is rejected at the edge; provider validators handle only plausible credentials; auth incident triage is faster because failures are classified (`missing` vs `malformed` vs `invalid`).
- Wild failure: parser rules are too strict and reject legitimate tokens, or fail-closed JWKS state causes long remote lockout during auth outages.
- Mitigations: table-driven parser tests for realistic token shapes, staged rollout behind provider-specific integration tests, explicit operator runbook for JWKS outage mode.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Wait for remote/07, then do all of Phase 06 at once | Lowest coordination cost now | Leaves known header-parsing hardening gap open longer; no incremental risk reduction |
| Pull remote/07 scope into this branch and ship everything together | Fastest path to full end-state if perfectly executed | Too broad (lfd + Concerto + CLI + auth server contracts), high merge/conflict risk, hard to validate safely |
| **Chosen: split Phase 06 into Slice A now + Slice B after remote/07** | Requires explicit staging discipline | Best risk-reduction per unit effort while preserving provider-isolation architecture |

## Key decisions

- **Decision: partial unblock now.** We will ship pre-validation independently instead of waiting for remote/07.
- **Decision: explicit parse states.** `Missing` and `Malformed` are distinct internal states to avoid auth ambiguity.
- **Decision: hard cap at 4096 bytes.** Large bearer headers are treated as malformed and denied.
- **Decision: no provider fallback ever.** Parse success only gates entry into the selected provider path; it never switches providers.
- **Decision: fail-closed JWKS contract.** When Studio trust is unavailable, Studio auth denies requests rather than soft-allowing.

## Scope

- In scope:
  - `rust/loopflow/src/lfd/auth.rs` token parser hardening
  - Auth middleware short-circuit for malformed bearer headers
  - Unit tests for parsing and middleware behavior
  - Phase-06 implementation notes for JWKS fail-closed + revocation semantics (to execute with remote/07)
- Out of scope:
  - Concerto sign-in UX and daemon picker
  - CLI device-code auth flow
  - Auth-server endpoint/schema changes beyond what remote/07 already tracks

## Done when

- `cargo test -p loopflow lfd::auth::tests::extract_token` passes with new malformed-token cases.
- A middleware test proves malformed Authorization headers are denied before provider validation.
- Existing Local/Static valid-token tests still pass unchanged.
- Design handoff for Slice B includes explicit JWKS unavailability behavior and revocation-window documentation tied to remote/07.
