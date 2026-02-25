# Security Wave — Blocked

The only remaining item is **Phase 06: Auth Provider Isolation** (`wave/security/06-auth-provider-isolation.md`).

Phase 06 is blocked on **remote/07 (Studio Auth)** which has not shipped yet. remote/07 adds JWKS validation to the `Studio` auth provider — Phase 06 hardens that path (fail-closed on JWKS issues, token format pre-validation, revocation documentation).

`wave/remote/07-studio-auth.md` still has open "What's left to build" sections (lfd JWKS validation, registration payload update, Concerto TokenProvider wiring, sign-in UI, auto-discovery, CLI device code auth).

## Options

1. **Wait** for remote/07 to ship, then pick Phase 06.
2. **Partial unblock**: Token format pre-validation (the `extract_token` hardening in Phase 06) could ship independently since it hardens existing header parsing without depending on JWKS. But the README explicitly marks 06 as blocked, so this would need explicit approval.

## Assumption used for kickoff

Proceed with option 2 for design work: split Phase 06 into a ship-now slice (token pre-validation) and a remote/07-dependent slice (JWKS fail-closed + revocation semantics).

## Open question

Is partial unblock (option 2) approved for implementation, or should we stop at design until remote/07 lands?
