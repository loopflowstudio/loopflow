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
