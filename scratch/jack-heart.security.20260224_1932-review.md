# Review — Auth Provider Isolation Slice A

## What was implemented

- Hardened `Authorization` parsing in `rust/loopflow/src/lfd/auth.rs` with explicit token states: `Missing`, `Malformed`, `Present`.
- Added provider-agnostic bearer pre-validation:
  - case-insensitive `Bearer` scheme required
  - non-empty token after trim
  - max token length `4096` bytes
  - reject whitespace/control characters in token
- Middleware now short-circuits malformed tokens to `401` before provider validation.
- Split provider auth flow into `authorize_with_provider` + `authorize_connection_token` to keep middleware dispatch clear.
- Added table-driven parser tests and middleware integration tests proving malformed Studio headers never call the validator.
- Updated `docs/lfd.md` to document malformed-header behavior and token constraints.

## Key choices

- **Explicit parse states over `Option<&str>`**: preserves semantic differences between missing and malformed auth headers.
- **Fail-fast malformed rejection**: avoids passing obviously invalid input into provider validators and keeps failure mode deterministic.
- **Provider-specific malformed error text**: keeps local/static (`malformed token`) distinct from Studio (`malformed connection token`) for operator triage.
- **Scope split**: this branch ships token pre-validation now; JWKS fail-closed and revocation-window docs stay with remote/07.

## How it fits together

`extract_token` is now the single gate for bearer-header shape and safety checks. `auth_middleware` consumes that parsed state, applies throttling using a stable auth-context hash, and only calls provider-specific validation when a token is `Present`. For Studio auth, malformed headers return immediately and skip `ConnectionValidator::validate` network calls.

## Risks and bottlenecks

- **Strictness risk**: malformed classification could reject unexpected but previously tolerated client header formatting.
- **Test harness overhead**: middleware tests spin up local HTTP servers; they are reliable today but slower than pure unit tests.
- **Remaining Studio trust work**: JWKS startup/refresh fail-closed behavior is intentionally out of this branch.

## What's not included

- JWKS validator implementation and key refresh semantics.
- Revocation latency/runbook documentation tied to JWKS cache policy.
- Concerto sign-in UX/device-code auth flow changes.
- Any provider fallback behavior changes beyond malformed-header short-circuiting.

## Validation run

- `cargo fmt --all -- --check`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test -p loopflow lfd::auth::tests`
- `cargo test --all` *(fails locally on docker-only tests because `/var/run/docker.sock` is unavailable in this environment; auth tests pass)*
