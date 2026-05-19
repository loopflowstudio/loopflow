## Try it!

```bash
cargo build --bin lf --bin lfd
uv run python scripts/test_pairing_smoke.py --timeout 10
```

Expected smoke output:

```text
PASS pair_url_shape
PASS paired_token_http
PASS paired_token_websocket
SUMMARY total=3 passed=3 failed=0
```

For the iOS compile path:

```bash
cd swift
xcodegen generate
xcodebuild build -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'generic/platform=iOS Simulator' CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO
```

Manual pairing examples:

```bash
lf op pair
lf op pair --host 100.64.1.2 --no-tls
lf op pair --host lfd.example.com --tls --tls-url https://lfd.example.com
```

## Intent

Make the first 30 seconds of mobile setup work without a Loopflow Studio account: run one command on the host, scan or paste one link on the phone, and reuse the existing remote connection, Keychain, pinning, HTTP, and WebSocket stack after that.

## Assumptions

- lfd still does not terminate TLS itself; plaintext pairing is only acceptable over Tailscale's `100.64.0.0/10` range.
- Accountless pairing can use `auth.mode=studio` with studio discovery disabled when no `~/.lf/credentials.json` exists.
- The durable phone credential for QR/paste is a 90-day lfd token; Studio discovery can keep its existing short-lived token pool model.
- iOS Simulator cannot scan camera QR codes, so smoke coverage uses paste/deep-link semantics plus an iOS build.

## Key decisions

- Added `TokenLedger::mint_with_ttl` and used a fixed 90-day pairing TTL instead of changing the global one-hour token TTL.
- Added `lf op pair` as an ops side-channel command that prints both terminal QR and `loopflow://pair` URL.
- Reused `loopflow://pair` rather than registering a second URL scheme.
- Centralized pairing application in `ConnectionStore.setPairingPayload(_:)` so token and optional certificate pin storage stay on the existing path.
- Documented `env -u LF_RUN_ID cargo test --all` for local wave-session gates because run-id-sensitive Rust tests assume a clean environment.

## Not included

- Wave cards or roadmap rendering.
- Any mobile write/build/land/chat actions.
- Native rustls/ACME support inside lfd.
- Sliding pairing-token renewal or token-kind audit migration.
- LAN fallback, Bonjour/mDNS, or plaintext outside Tailscale.
