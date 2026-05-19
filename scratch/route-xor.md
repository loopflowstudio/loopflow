path: done

Ship-ready. The branch delivers the full `remote-lfd-connection` intent end to
end: `lf op pair` mints a 90-day ledger token and prints a terminal QR + URL,
iOS gains the single setup screen (Scan QR / Paste link / Sign in with
Loopflow), `loopflow://pair` deep links route into the existing connection
stack, and WS close 4401 surfaces a re-pair state instead of a spinner.

Gate polish after the first route kept scope unchanged and hardened one host
side edge: `--tls-url` no longer builds a shell pipeline or depends on `xxd`;
it fetches/converts the leaf cert with `openssl` and hashes DER bytes in Rust.

Validation re-run on the current tree:

- `cargo fmt --check` — pass
- `cargo clippy -p loopflow -- -D warnings` — pass
- `env -u LF_RUN_ID cargo test -p loopflow pair --lib` — pass (12 tests)
- `env -u LF_RUN_ID cargo test --all` — pass
- `uv run pytest python/tests/` — pass (137 tests)
- `swift test --package-path swift` — pass (10 XCTest + 338 Swift Testing tests)
- `swift test --package-path swift --filter PairingPayload` — pass (5 tests)
- `uv run python scripts/check_swift_multiplatform_boundaries.py` — pass
- `uv run python scripts/test_pairing_smoke.py --timeout 10` — pass (`pair_url_shape`,
  `paired_token_http`, `paired_token_websocket`)
- `cd swift && xcodegen generate && xcodebuild build -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'generic/platform=iOS Simulator' CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO` — pass
- `tests/e2e/test_smoke.sh` — pass
- `uv run pytest tests/e2e/test_api_smoke.py tests/e2e/test_concurrent_clients.py -v` — pass (16 tests)
- `cd swift && xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' -only-testing:ConcertoTests CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO` — pass (338 Swift Testing tests)

A full local `ConcertoUITests` xcodebuild run was attempted and interrupted
after the runner hung locally; no branch-specific failure surfaced before the
hang. The iOS build and Swift package tests cover the mobile compile path that
previously missed iOS-only errors. Scope held to the view-only charter — no
write/build/land/chat surfaces. No reason to iterate.
