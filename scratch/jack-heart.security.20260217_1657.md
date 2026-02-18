# Security hardening: current state

## Scope

This branch consolidated security hardening for `lfd` with two goals:

1. Prevent filesystem escape from user-controlled identifiers/paths.
2. Require local mutation auth via session token without breaking local developer UX.

## Shipped in this branch

- Added centralized path-security helpers in `rust/loopflow/src/lfd/security.rs`:
  - root-constrained resolution for existing paths (`path_within_root_existing`)
  - root-constrained resolution for planned paths (`path_within_root_planned`)
  - strict path-component ID validation (`validate_safe_id`)
  - filesystem component sanitization for derived names
- Routed current filesystem touchpoints through these guards:
  - `OutputHub` log read/write path resolution
  - sqlite path resolution from `LFD_DB_PATH` (now constrained under `~/.lf`)
  - worktree path/name derivation
  - git hook repo path canonicalization and validation
- Added local session-token generation and enforcement:
  - token persisted at `~/.lf/session-token` with mode `0600`
  - local loopback reads remain allowed, but mutation methods require token
- Updated clients for local token discovery:
  - Python: `LFD_TOKEN` first, then `~/.lf/session-token` for local base URLs
  - Swift: explicit token → static token → local file fallback
- Updated docs/plans to reflect shipped behavior:
  - `docs/lfd.md`
  - `wave/remote/08-api-expansion.md`
  - `wave/security/02-path-validation.md`
  - `wave/security/README.md`

## Durable decisions

- Use centralized fail-closed validation for filesystem paths instead of handler-local checks.
- Canonicalize and enforce root boundaries to block symlink-based escape.
- Treat auth by method tier (read-only vs mutation) in middleware.
- Keep local token fallback local-only to avoid credential leakage to remote hosts.
- Reject absolute sqlite override paths and constrain DB location under `~/.lf`.

## Remaining follow-ups (not in scope here)

- Phase 03: container/runtime isolation hardening.
- Phase 04: rate limits and API surface limiting.
- Phase 06: provider-isolated loopback policy and JWKS hardening.
- Phase 08: file API implementation (path-validation contract is documented, endpoint not yet shipped).

## Verification run

- `cargo fmt --all -- --check`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test --all`
- `uv run pytest python/tests/`
- `swift test --package-path swift`
- `cd swift && xcodegen generate && xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS'`
- `tests/e2e/test_smoke.sh`

All passed locally.
