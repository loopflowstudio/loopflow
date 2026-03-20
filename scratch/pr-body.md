## Try it!

```bash
cargo test --all
uv run pytest python/tests/
tests/e2e/test_smoke.sh
uv run pytest tests/e2e/test_api_smoke.py tests/e2e/test_concurrent_clients.py -v
swift test --package-path swift
cd swift && xcodegen generate
tmpdir=$(mktemp -d /tmp/loopflow-xcode.XXXXXX) && xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' -derivedDataPath "$tmpdir" CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO -only-testing:ConcertoTests
./dev run-debug
```

Then in Concerto:
- open a repo card from the portfolio window and confirm the repo falls back to the attention queue when no wave is selected
- select a wave and confirm Work stays the default surface while Terminal appears only when that wave has an active terminal session
- focus an embedded terminal pane and verify ordinary typing still feels character-by-character, while pane shortcuts like `⌃⇧5`, `⌃⇧'`, `⇧⌘↩`, `⌘W`, and `⌥⌘←/→` stay app-owned
- open the command palette and confirm the pane actions show the same shortcuts the keyboard router actually handles

## Intent

Unify Loopflow's daemon runtime and Concerto's macOS UI around first-class wave workspaces. This branch makes terminal sessions durable daemon state, surfaces them throughout the HTTP/event model, and lets Concerto compose portfolio, attention, and workspace UI around tmux-backed embedded terminals instead of a Swift-only terminal shim. It also aligns builtin flows and PM-aware operations with that run-centric model so automation, review, and UI all describe the same execution graph.

## Assumptions

- `lfd` remains the source of truth for wave, run, attention, and terminal-session state; Concerto renders and acts on that shared state instead of inventing parallel session models.
- Reviewers validating `ConcertoTests` locally should prefer a fresh `-derivedDataPath`; on this host the shared default DerivedData path intermittently reused broken UITest build products.
- Ghostty, tmux, and local macOS app permissions are available for the embedded terminal path.
- The screenshot UI-test harness is still environment-sensitive: `ConcertoUITests/ScreenshotPipelineTests/testCapture` crashes the runner before XCTest connects on this machine.

## Key decisions

- Persist terminal sessions in `lfd` and expose them through dedicated routes/events rather than treating embedded terminals as app-local state.
- Model the workspace as a persisted binary split tree per wave (`MultiplexerLayout`/`MultiplexerStore`) so layout survives refreshes and restarts.
- Keep Ghostty on its native key-event path; only explicit pane-management shortcuts are intercepted above the terminal.
- Move builtin flow naming toward `build`, `garden`, `wave`, and VSM governance concepts so daemon automation, PM sync, and Concerto copy all use the same vocabulary.
- Use keycode-based split bindings (`⌃⇧5`, `⌃⇧'`) and read shortcut labels from `KeyboardRouter` so palette/help text cannot drift from actual routing.

## Not included

- Remote daemon-owned PTY transport
- Final wave-hierarchy algedonic escalation routing
- A stable fix for the macOS screenshot/UI harness bootstrap crash
- Named saved workspace layouts or additional pane types beyond terminal/markdown/diff/launchpad
