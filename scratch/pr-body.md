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

- waiting waves now round-trip through persisted `terminal_sessions` in `lfd`, with attach/start/complete/cancel routes instead of ad-hoc local session state
- a selected wave now behaves like its own workspace: terminal tabs, runs, attention, and multiplexer layout stay scoped to that wave
- multiplexer shortcuts route by focus: native panes split/close in SwiftUI, terminal-pane actions go to tmux
- attention items round-trip as the collapsed `interactive` / `algedonic` model across daemon, Python client, Swift models, and UI
- UI-test and snapshot launches skip eager daemon/voice warmup, reducing startup side effects during automation

Validation on March 19, 2026:

- `cargo fmt --check` ✅
- `cargo clippy -- -D warnings` ✅
- `cargo test --all` ✅
- `uv run pytest python/tests/` ✅
- `tests/e2e/test_smoke.sh` ✅
- `uv run pytest tests/e2e/test_api_smoke.py tests/e2e/test_concurrent_clients.py -v` ✅
- `swift test --package-path swift` ✅
- `xcodebuild test` ⚠️ `ConcertoUITests-Runner` killed before establishing connection; app and non-UI suites pass
