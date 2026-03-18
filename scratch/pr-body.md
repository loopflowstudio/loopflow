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

## Intent

Make paused waves runnable from the selected-wave header while keeping Concerto's native workspace front and center. This change adds explicit terminal-session lifecycle tracking in `lfd`, wires the Swift client to that state, and turns the selected-wave view into a workspace where terminal embedding is additive rather than a takeover.

## Assumptions

- Interactive terminal embedding is currently a local-repo/macOS path backed by Ghostty.
- `lfd` remains the authority for wave and terminal-session lifecycle even though the local attach path still launches a wrapped shell command rather than streaming a daemon-owned PTY.
- Reviewers validating the full UX have a rendered macOS environment; headless `xcodebuild test` still loses the UI runner before it establishes a connection.

## Key decisions

- Added a first-class `terminal_sessions` store/API/event path instead of inferring terminal state from generic wave waiting status.
- Kept run overrides one-shot (`runWave(..., overrides: ...)`) so header selections do not rewrite the wave's saved default flow.
- Made the terminal tab conditional per selected wave and preserved the native Work view as the default surface.
- Scoped the transport seam so a future daemon-owned PTY path can replace the current local launch spec without rewriting the UI state model.

## Not included

- Server-owned PTY transport or collaborative terminal input.
- Durable scrollback/reconnect across app restarts.
- Remote terminal embedding for non-local repos.
- Replacing non-interactive execution with terminal sessions.

## Validation

- `cargo fmt --check` ✅
- `cargo clippy -- -D warnings` ✅
- `cargo test --all` ✅
- `uv run pytest python/tests/` ✅ (113 passed)
- `swift test --package-path swift` ✅ (10 XCTest + 246 Swift Testing cases)
- `tests/e2e/test_smoke.sh && uv run pytest tests/e2e/test_api_smoke.py tests/e2e/test_concurrent_clients.py -v` ✅ (smoke pass + 16 passed)
- `xcodebuild test ...` ✅ for build + package/unit coverage, but the UI runner still exited early before establishing connection in this no-rendering environment
