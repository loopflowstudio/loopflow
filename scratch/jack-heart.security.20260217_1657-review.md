# Security hardening review (gate)

## What was implemented

- Added centralized path-security utilities in `rust/loopflow/src/lfd/security.rs`:
  - root-constrained resolution for existing and planned paths
  - strict ID validation for path components
  - filesystem-component sanitization for derived names
- Applied those guards to current filesystem touchpoints:
  - `OutputHub` log reads/writes (`wave_run_id` validation + root checks)
  - sqlite path resolution from `LFD_DB_PATH` (now constrained under `~/.lf`)
  - worktree name/path derivation for wave/CI fix worktrees
  - git hook repo path canonicalization and validation
- Added local session-token generation/wiring for `AuthProvider::Local`:
  - token written to `~/.lf/session-token` (Unix mode `0600`)
  - middleware now allows loopback read-only requests but requires token for mutations
- Updated clients to consume local session token automatically when appropriate:
  - Python client reads `LFD_TOKEN` first, then `~/.lf/session-token` for local base URLs
  - Swift services resolve token via explicit token → static token → local file fallback
- Updated docs and security planning artifacts:
  - `docs/lfd.md` auth and sqlite path semantics
  - `wave/remote/08-api-expansion.md` requires path guards before file access
  - restored `wave/security/02-path-validation.md` as a shipped-phase doc and marked phase 02 done in `wave/security/README.md`

## Key choices

- **Centralized fail-closed validation** over per-handler checks to prevent drift and missed endpoints.
- **Canonicalize then enforce root-prefix** to catch symlink escapes, not just string traversal.
- **Method-based auth tiering** (`GET/HEAD/OPTIONS` vs mutation verbs) for simpler enforcement in middleware.
- **Local-only token-file fallback in clients** to avoid leaking machine-local session credentials to remote servers.

Alternatives rejected: sanitize-only path joining, ad-hoc inline checks, and preserving absolute sqlite override behavior.

## How it fits together

`lfd` now routes filesystem-bound inputs through `security.rs` helpers before filesystem I/O. Auth middleware classifies requests by method and provider, then checks either session token (local) or configured provider token. Python/Swift clients automatically source local session tokens for local daemon usage, so secured local mutations remain seamless.

## Risks and bottlenecks

- Loopback read bypass still applies for non-local providers (`Static`/`Studio`), documented as remaining Phase 06 work.
- `LFD_DB_PATH` now requires a path under `~/.lf`; environments relying on absolute override paths must migrate.
- Planned-path validation requires canonicalizable parents; nested sqlite override paths require parent directory creation first.

## What's not included

- Phase 03 container runtime isolation/hardening.
- Phase 04 rate limiting and API surface limits.
- Phase 06 provider-isolated loopback policy/JWKS hardening.
- Phase 08 file API implementation itself (only the mandatory path-validation contract is documented).

## Verification run

- `cargo fmt --all -- --check`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test --all`
- `uv run pytest python/tests/`
- `swift test --package-path swift`
- `cd swift && xcodegen generate && xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS'`
- `tests/e2e/test_smoke.sh`

All passed locally.
