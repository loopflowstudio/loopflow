## Try it!

```bash
# Core validation
cargo fmt --check
cargo clippy -- -D warnings
cargo test --all
cargo test -p loopflow docker_
uv run pytest python/tests/
tests/e2e/test_smoke.sh
uv run pytest tests/e2e/test_api_smoke.py tests/e2e/test_concurrent_clients.py -v
swift test --package-path swift

# Concerto app + Xcode coverage
uv run python scripts/concerto-dev.py run-debug
cd swift && xcodegen generate && xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO
```

In Concerto:
- open a repo window with no selected wave and confirm the empty queue shows wave overview cards instead of the old detail-panel empty state
- select a wave and confirm the default workspace opens with **Roadmap**, **Runs**, and **Terminal** panes
- press **Cmd+K** and switch waves or focus/create panes like **README**, **Launcher**, and extra **Terminal** panes
- drive a wave into an interactive step and confirm the queue item shows step-specific detail, the workspace auto-takes over with the live session, and the item resolves when the session finishes

## Intent

Ship the new Concerto workspace model end to end: `lfd` now exposes persistent terminal sessions and interactive-attention lifecycle hooks, while the macOS app replaces the single detail panel with a per-wave multiplexer that still yields immediately to live interactive sessions. Reviewers should be able to evaluate both the backend contract and the user-facing workflow in one pass.

## Assumptions

- Concerto can rely on a local `lfd` HTTP/WebSocket endpoint to create, attach, and complete terminal sessions.
- Interactive-step previews come from on-disk artifacts (`scratch/*.md`, mutation summaries) rather than from prompt text.
- Per-wave layout/session selection persistence in `UserDefaults` is the right scope for workspace state; cross-device/session sync is out of scope.
- Ghostty-backed terminal panes remain optional shell surfaces: waves without worktrees still render roadmap/readme/runs panes and show placeholders for terminal-style panes.

## Key decisions

- Keep attention kinds coarse (`interactive`, `algedonic`) and put step-specific rendering behind typed context fields.
- Let the executor, not individual steps, own attention-item creation/resolution.
- Persist multiplexer layout per repo/wave and terminal-session selection per repo so workspace chrome survives refreshes without overriding active interactive sessions.
- Use the queue empty state as a lightweight wave overview/dashboard rather than a dead-end blank panel.

## Not included

- Full tmux-per-wave dashboard embedding; this branch stops at terminal-session plumbing and multiplexer UI.
- Calibration-specific queue views or new attention kinds.
- A machine-specific fix for the `ConcertoUITests-Runner` early bootstrap exit seen in `xcodebuild test` here.

## Validation

Passed locally on March 20, 2026:

- `cargo fmt --check`
- `cargo clippy -- -D warnings`
- `cargo test --all`
- `cargo test -p loopflow docker_`
- `uv run pytest python/tests/`
- `tests/e2e/test_smoke.sh`
- `uv run pytest tests/e2e/test_api_smoke.py tests/e2e/test_concurrent_clients.py -v`
- `swift test --package-path swift`

Attempted locally on March 20, 2026:

- `cd swift && xcodegen generate && xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO`
  - app/unit suites ran, but `ConcertoUITests-Runner` exited early before finishing bootstrap on this machine
