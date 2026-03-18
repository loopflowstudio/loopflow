## Try it!

```bash
cargo fmt --check
cargo clippy -- -D warnings
cargo test --all
uv run pytest python/tests/
swift test --package-path swift
tests/e2e/test_smoke.sh && uv run pytest tests/e2e/test_api_smoke.py tests/e2e/test_concurrent_clients.py -v
cd swift && xcodegen generate && xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' -derivedDataPath /tmp/LoopflowSwiftGate.$(date +%s) CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO
```

For the product flow:

```bash
uv run python scripts/concerto-dev.py run-debug
```

With two paused local waves, verify:

- selecting a wave opens a workspace surface instead of taking over the whole window with a terminal
- Work stays the default tab for the selected wave
- changing the header typeahead to `design` launches that override for a paused wave
- a Ghostty-backed Terminal tab appears only when that wave has an active tracked terminal session
- exiting the terminal with status `0` resumes the wave in `lfd`; non-zero fails it
- with no selected wave, the repo window still lands on the live attention queue

## Validation

- `cargo fmt --check` ✅
- `cargo clippy -- -D warnings` ✅
- `cargo test --all` ✅
- `uv run pytest python/tests/` ✅ (113 passed)
- `swift test --package-path swift` ✅ (10 XCTest + 246 Swift Testing cases)
- `tests/e2e/test_smoke.sh && uv run pytest tests/e2e/test_api_smoke.py tests/e2e/test_concurrent_clients.py -v` ✅ (smoke pass + 16 passed)
- `xcodebuild test ...` ✅ for build + package/unit coverage, but the UI runner still exited early before establishing a connection in this no-rendering environment
