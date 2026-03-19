## Try it!

```bash
cargo fmt --check
cargo clippy -- -D warnings
cargo test --all
uv run pytest python/tests/
tests/e2e/test_smoke.sh
uv run pytest tests/e2e/test_api_smoke.py tests/e2e/test_concurrent_clients.py -v
swift test --package-path swift
cd swift && xcodegen generate && xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO
```

What to look for:

- `lfd` now persists `terminal_sessions` and exposes attach/start/complete/cancel routes for them.
- Concerto wave detail views now behave like workspaces: runs, attention, and tracked terminal tabs live together.
- Attention items now round-trip as the collapsed `interactive` / `algedonic` model across daemon, Python client, Swift models, and UI.
- The generated macOS app now resolves bundled frameworks from `Contents/Frameworks`, so a locally built Concerto app launches outside the package-test harness.

Validation from this gate:

- `cargo fmt --check` ✅
- `cargo clippy -- -D warnings` ✅
- `cargo test --all` ✅
- `uv run pytest python/tests/` ✅
- `tests/e2e/test_smoke.sh` ✅
- `uv run pytest tests/e2e/test_api_smoke.py tests/e2e/test_concurrent_clients.py -v` ✅ (16 passed)
- `swift test --package-path swift` ✅
- direct launch of the built app ✅
  - `/Users/jack/Library/Developer/Xcode/DerivedData/LoopflowSwift-fhniguselvodfhbnlvqizehygmun/Build/Products/Debug/Concerto.app/Contents/MacOS/Concerto -ui-test-mode mock-waves`
- `xcodebuild test ...` ⚠️ after a clean DerivedData rebuild, the app and unit/package tests pass but local `ConcertoUITests-Runner` still exits during bootstrap before establishing the UI-test connection
