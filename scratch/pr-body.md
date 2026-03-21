## Try it!

```bash
# Core validation
cargo test --all
uv run pytest python/tests/
swift test --package-path swift

# API / websocket smoke
tests/e2e/test_smoke.sh
uv run pytest tests/e2e/test_api_smoke.py tests/e2e/test_concurrent_clients.py -v

# Run the app from this branch
uv run python scripts/concerto-dev.py run-debug
```

In Concerto:
- select a wave and confirm the old scroll-view detail panel is gone
- the default workspace should open with **Roadmap**, **Runs**, and **Terminal** panes
- press **Cmd+K** and switch waves or open/focus **README** / **Launcher** panes
- pick a waiting wave and confirm the interactive session still takes over instead of leaving you stranded in the workspace

## Validation

Passed locally on March 20, 2026:

- `cargo fmt --check`
- `cargo clippy -- -D warnings`
- `cargo test --all`
- `uv run pytest python/tests/`
- `swift test --package-path swift`
- `tests/e2e/test_smoke.sh`
- `uv run pytest tests/e2e/test_api_smoke.py tests/e2e/test_concurrent_clients.py -v`

Attempted locally on March 20, 2026:

- `cd swift && xcodegen generate && xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO`
  - the app/unit suites ran, but `ConcertoUITests-Runner` exited early before finishing bootstrap on this machine
