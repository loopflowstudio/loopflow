# Validation

Passed locally:

- `cargo fmt --check`
- `cargo clippy -- -D warnings`
- `cargo test --all`
- `uv run pytest python/tests/`
- `tests/e2e/test_smoke.sh`
- `uv run pytest tests/e2e/test_api_smoke.py tests/e2e/test_concurrent_clients.py -v`
- `docker version`
- `cargo test -p loopflow docker_`
- `swift test --package-path swift`
- `cd swift && xcodegen generate`
- `tmpdir=$(mktemp -d /tmp/loopflow-xcode.XXXXXX) && cd swift && xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' -derivedDataPath "$tmpdir" CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO -only-testing:ConcertoTests`

Known failing on this host:

- `tmpdir=$(mktemp -d /tmp/loopflow-xcode-ui.XXXXXX) && cd swift && xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' -derivedDataPath "$tmpdir" CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO -only-testing:ConcertoUITests/ScreenshotPipelineTests/testCapture`
  - `ConcertoUITests-Runner` exits with `signal kill` before XCTest connects.

## Try it

In Concerto (`./dev run-debug`):
- Open a repo card from the portfolio window → confirm repo falls back to attention queue when no wave is selected
- Select a wave → confirm Work stays default surface, Terminal appears only when that wave has an active terminal session
- Focus an embedded terminal pane → verify typing feels character-by-character; pane shortcuts (`⌃⇧5`, `⌃⇧'`, `⇧⌘↩`, `⌘W`, `⌥⌘←/→`) stay app-owned
- Open command palette → confirm pane actions show the same shortcuts the keyboard router handles
