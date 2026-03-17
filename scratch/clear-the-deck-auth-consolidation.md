# Clear the Deck: Auth Cleanup

## Problem

Auth config uses `auth.provider` (implementation-leaking name) with a `static` mode that doesn't describe its use case. The Swift app has a manual connection path (enter IP:port + token) that nobody uses — discovery via studio is the path. Dead weight.

## Approach

Rename config fields, delete manual connection UI, close out the empty growth cleanup item.

### Changes

**Rust — config rename:**
- `auth.provider` → `auth.mode` in `AuthConfig` struct and all references
- `Static` → `Ci` in `AuthProvider` enum
- `"static"` → `"ci"` in config parsing, env var handling (`LFD_AUTH_PROVIDER` accepts both for transition)
- `DEFAULT_AUTH_BASE_URL` and `base_url` field stay (studio still uses them)

**Swift — delete manual connection:**
- Delete `ConnectionSetupView.swift` (manual IP:port entry)
- Remove manual connection link from `DiscoveryView.swift`
- `ConnectionStore` and `ServerConnection` stay — discovery path creates these programmatically
- `ConnectionAuthMode` stays — discovery uses `.staticToken` with connection tokens

**Wave items:**
- Close `wave/clear-the-deck/04-growth-cleanup.md` — no growth code exists in codebase
- Auth consolidation (team mode) moved to `wave/trust/06-team-auth.md` as backlog

### Files to change

| File | Change |
|------|--------|
| `rust/loopflow/src/lfd/config.rs` | `provider` → `mode`, `"static"` → `"ci"`, keep `"static"` as deprecated alias |
| `rust/loopflow/src/lfd/auth.rs` | `Static` → `Ci` variant |
| `rust/loopflow/src/lfd/mod.rs` | Update match arms for renamed variant |
| `rust/loopflow/src/lfd/service/compose.rs` | Update auth provider construction |
| `rust/loopflow/src/bin/lfd.rs` | Update config references |
| `swift/Concerto/Platform/iOS/ConnectionSetupView.swift` | Delete |
| `swift/Concerto/Platform/iOS/DiscoveryView.swift` | Remove manual connection link |
| `wave/clear-the-deck/04-growth-cleanup.md` | Mark done or delete |
| Tests referencing `Static` or `provider` | Update |

### What stays unchanged

- `Local` and `Studio` auth modes — behavior identical
- `registration.rs`, `token_ledger.rs`, `credentials.rs` — all stay
- Discovery flow (DiscoveryService, DiscoveryView) — untouched
- Provider auth (GitHub/Claude/Codex/Zen) — orthogonal
- All executor code (Docker, Sandbox, Adaptive) — untouched

## Done when

- `cargo test --all` passes
- `cargo fmt --check` passes
- `cargo clippy -- -D warnings` passes
- `swift test --package-path swift` passes
- Config accepts `auth.mode: local | studio | ci` (and `static` as deprecated alias)
- No `ConnectionSetupView.swift` in codebase
- No manual connection entry point in DiscoveryView
