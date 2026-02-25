# Auth Token Pre-Validation — Shipped

## What shipped

- `Authorization` parsing now classifies `Missing`, `Malformed`, and `Present`.
- Malformed bearer values are denied with `401` **before** provider validation.
- Bearer requirements are explicit: `Bearer <token>`, non-empty token, max 4096 bytes, no embedded whitespace/control characters.
- Studio-specific tests confirm malformed headers do not call `ConnectionValidator::validate`.

## Remaining JWKS/JWT work folded into remote/07

The security wave is retired. Remaining hardening (JWKS fail-closed, JWT claim validation, operator docs) now lives in `wave/remote/07-studio-auth.md`.
