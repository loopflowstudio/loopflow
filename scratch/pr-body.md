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

- ✅ `cargo fmt --check`
- ✅ `cargo clippy -- -D warnings`
- ✅ `cargo test --all`
- ✅ `uv run pytest python/tests/`
- ✅ `tests/e2e/test_smoke.sh`
- ✅ `uv run pytest tests/e2e/test_api_smoke.py tests/e2e/test_concurrent_clients.py -v`
- ✅ `swift test --package-path swift`
- ⚠️ `xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' ...` still fails locally because `ConcertoUITests-Runner` is killed before establishing its connection; the app and non-UI Swift suites complete first

## Intent

Move interactive wave work from an ad-hoc embedded terminal into a daemon-backed terminal-session model and give each wave a real workspace. The change makes waiting runs durable in `lfd`, keeps terminal state attached to the wave that owns it, and introduces the first native outer multiplexer so Concerto can combine tmux-backed shell work with lightweight native panes.

## Assumptions

- Each wave should own exactly one outer terminal pane, and tmux should remain the owner of inner shell splits and windows.
- Reviewers have tmux available locally when validating terminal-pane commands.
- The remaining `ConcertoUITests-Runner` bootstrap failure is a host/UI-automation issue rather than a regression in the shipped app path; that is an inference from the local run and should be validated in CI.

## Key decisions

- Persist terminal sessions in `lfd` and expose explicit lifecycle routes rather than keeping interactive waits only in client memory.
- Keep the existing shortcut names and make them focus-sensitive instead of introducing separate native-pane vs tmux keymaps.
- Persist multiplexer layout and per-wave terminal-tab selection locally so switching waves preserves context.
- Treat UI-test and snapshot launches as automated contexts so app warmup work does not introduce flaky automation behavior.

## Not included

- Native inner-terminal pane management beyond tmux
- Rich native editors/viewers for markdown or diff panes
- A pane-type picker; native pane splitting still follows the fixed markdown → diff → launchpad cycle
