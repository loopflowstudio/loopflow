## Validation

Passed:

- `cargo fmt --check`
- `cargo clippy -- -D warnings`
- `cargo test --all`
- `uv run pytest python/tests/`
- `tests/e2e/test_smoke.sh`
- `uv run pytest tests/e2e/test_api_smoke.py tests/e2e/test_concurrent_clients.py -v`
- `swift test --package-path swift`
- direct launch of the built macOS app after the runpath fix:
  - `/Users/jack/Library/Developer/Xcode/DerivedData/LoopflowSwift-fhniguselvodfhbnlvqizehygmun/Build/Products/Debug/Concerto.app/Contents/MacOS/Concerto -ui-test-mode mock-waves`

Still failing locally after a clean DerivedData rebuild:

- `cd swift && xcodegen generate && xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO`
  - app build succeeds
  - unit/package tests inside the run pass
  - `ConcertoUITests-Runner` still exits during bootstrap before establishing the UI-test connection
