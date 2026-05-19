# Gate review — mobile remote lfd connection

## What was implemented

Added the accountless remote-lfd pairing path for Loopflow mobile:

- `lf op pair` mints a 90-day connection-token ledger entry, resolves a reachable host, prints a terminal QR, and prints the matching `loopflow://pair?...` link.
- iOS now has a single setup screen with Scan QR, Paste pairing link, and Sign in with Loopflow paths.
- `PairingPayload` in `LoopflowCore` parses the QR/deep-link payload, rejects unsafe plaintext, normalizes optional certificate pins, and feeds the existing `ConnectionStore` / Keychain path.
- The app handles `loopflow://pair` deep links, reconnects on foreground, and maps WebSocket close code `4401` to a re-pair auth failure.
- `scripts/test_pairing_smoke.py` proves the pairing token works for HTTP `/v0/waves` and `/ws` without a physical phone.

## Key choices

- Pairing tokens use a fixed 90-day TTL through `TokenLedger::mint_with_ttl`; no schema migration or sliding renewal loop.
- Plaintext is allowed only for Tailscale `100.64.0.0/10` hosts. Non-Tailscale hosts default to TLS and can carry a QR-provided certificate pin.
- `loopflow://pair` reuses the existing registered app scheme instead of adding `lfd://`.
- `auth.mode=studio` can run without studio credentials; the local token ledger remains available for accountless pairing while discovery stays disabled.
- Gate polish changed duplicate pairing query fields from a Swift dictionary trap into a typed `invalidField` error.

## How it fits together

The host-side CLI and phone-side parser share one wire shape: `loopflow://pair?host&port&tls&token&fp`. `lf op pair` writes/mints from the same local ledger that lfd validates in studio mode, while iOS stores the token through the existing `ConnectionStore`/`ConnectionSecretStore` path and then calls the existing `connectLfd` flow. The smoke script exercises the same contract without camera hardware by parsing the emitted link and using the token against live HTTP and WebSocket endpoints.

## Risks and bottlenecks

- QR scanning needs a physical iPhone for full camera validation; Simulator coverage comes through paste/deep-link and the iOS build.
- `--tls-url` shells out to `openssl`; hosts without `openssl`/`xxd` need to pass `--fingerprint` directly.
- `EventService` currently maps `4401` to a re-pair message for all paths. Full silent studio rediscovery remains a follow-up.
- Full Rust tests inherit `LF_RUN_ID` under wave sessions and fail two run-id-sensitive tests unless run with `env -u LF_RUN_ID`; TESTING.md now documents that local gate command.

## What's not included

- Wave list, roadmap, task detail, or other read surfaces.
- Mobile write/build/land/chat actions.
- Native TLS termination in lfd.
- Sliding token renewal or a `token_kind` audit column.
- mDNS/LAN discovery or a non-Tailscale plaintext fallback.

## Validation

- `cargo fmt --check` — pass.
- `cargo clippy -p loopflow -- -D warnings` — pass.
- `cargo test -p loopflow pair --lib` — pass (11 tests selected by filter).
- `env -u LF_RUN_ID cargo test --all` — pass (full Rust suite; unsetting run id is required inside wave sessions).
- `swift test --package-path swift` — pass (10 XCTest + 338 Swift Testing tests).
- `swift test --package-path swift --filter PairingPayload` — pass (5 pairing tests).
- `uv run python scripts/check_swift_multiplatform_boundaries.py` — pass.
- `uv run python scripts/test_pairing_smoke.py --timeout 10` — pass (`pair_url_shape`, `paired_token_http`, `paired_token_websocket`).
- `cd swift && xcodegen generate && xcodebuild build -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'generic/platform=iOS Simulator' CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO` — pass.

