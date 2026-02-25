# Phase 06A: Auth Provider Isolation (Unblock Plan)

## Problem

Phase 06 is the only remaining security-wave hardening item, but it is blocked on remote/07. Waiting leaves a known auth gap: malformed `Authorization` bearer values can still reach provider validators.

This design advances two explicit goals from `wave/security/README.md`:

- "No unauthenticated mutation"
- "Fail closed on auth/trust ambiguity"

Who benefits now:
- Remote operators: malformed auth traffic is denied early and predictably.
- Security reviewers: clear separation between `missing`, `malformed`, and `invalid` auth failures.
- Future remote/07 work: JWKS hardening lands on top of a stricter middleware boundary.

## Approach

Ship Phase 06 in two slices instead of waiting for one large remote/07 merge.

### Slice A (ship now): provider-agnostic token pre-validation in middleware

1. Replace `extract_token(&HeaderMap) -> Option<&str>` with explicit parse states:
   - `Missing`
   - `Malformed`
   - `Present(&str)`
2. Enforce edge validation before any provider call:
   - `Bearer <token>` scheme (case-insensitive)
   - non-empty token after trim
   - max length 4096 bytes
   - reject control characters (including `\0`) and embedded whitespace
3. Middleware behavior:
   - `Missing` => existing missing-token path
   - `Malformed` => immediate 401, no provider validation call
   - `Present` => existing provider-specific validation path
4. Add table-driven parser tests and middleware short-circuit tests in `rust/loopflow/src/lfd/auth.rs`.

### Slice B (with remote/07): JWKS fail-closed + revocation semantics

When `JwksValidator` lands for Studio auth:
- Startup: retry JWKS fetch with backoff; daemon starts, but Studio auth denies until keys are available.
- Refresh: reuse last-known-good keyset on transient failures; deny when keyset exceeds max staleness window.
- JWT checks: reject `alg:none`; validate `iss`, `aud`, `exp`, `nbf` (30s skew).
- Document effective revocation window from cache + refresh behavior.

### Wild success

Auth failures become sharply classified and cheap to triage; provider validators only process plausible credentials; remote/07 can focus on JWKS trust correctness instead of header parsing.

### Wild failure

Parser rules are too strict and reject legitimate bearer values, or JWKS fail-closed behavior causes prolonged lockout during auth-service outages.

Mitigation:
- realistic parser fixtures from existing client token shapes
- explicit operator runbook for JWKS outage mode
- integration coverage for Local/Static/Studio provider isolation

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Wait for remote/07, then ship all of Phase 06 together | Lowest coordination now | Leaves known malformed-token gap open longer with no incremental hardening |
| Pull remote/07 into this branch and ship end-to-end | Fastest theoretical full state | Too broad (lfd + Concerto + CLI + auth-server contracts), high merge and validation risk |
| **Chosen: split into Slice A now + Slice B with remote/07** | Requires strict staging discipline | Best risk reduction per unit effort while preserving provider isolation |

## Key decisions

- **Partial unblock now.** Token pre-validation ships independently.
- **Explicit parse states.** `Missing` and `Malformed` are distinct internal outcomes.
- **4096-byte hard cap.** Oversized bearer headers are denied as malformed.
- **No provider fallback.** Parse success gates entry only into configured provider path.
- **Fail-closed JWKS contract.** Studio trust ambiguity denies auth by default.

## Scope

- In scope:
  - `rust/loopflow/src/lfd/auth.rs` token parser hardening
  - middleware short-circuit for malformed headers
  - parser + middleware tests for edge cases
  - remote/07 handoff notes for JWKS fail-closed + revocation documentation
- Out of scope:
  - Concerto sign-in UX/daemon picker
  - CLI device-code auth flow
  - auth-server schema changes outside remote/07

## Done when

- `cargo test -p loopflow lfd::auth::tests::extract_token` passes with malformed-token cases.
- A middleware test proves malformed `Authorization` headers return 401 before provider validation.
- Existing valid-token Local/Static tests continue passing unchanged.
- remote/07 design includes explicit JWKS unavailability behavior + revocation-window documentation, advancing wave goals: "No unauthenticated mutation" and "Fail closed on auth/trust ambiguity".
