# Review: jack-heart.agent-embedding.20260317_1347

## What was implemented

- Added lfd-backed terminal session records, persistence, migration, HTTP routes, and event payloads so interactive wave runs have an explicit lifecycle instead of piggybacking on generic waiting state.
- Updated wave execution so paused waves can be manually run with a one-shot step/flow override, terminal-session completion drives wave resume/failure, and attention items surface richer backend kinds.
- Reworked Concerto's selected-wave UI into a workspace-first layout: Work stays the default tab, Terminal appears only when that wave has a live tracked session, and the terminal sidebar keeps recent run/PR/queue context visible.
- Updated Swift client models/state/services and tests so the app can create, attach, start, and observe tracked terminal runs from the selected wave header.

## Key choices

- Kept `lfd` as the source of truth for terminal-session state even though the current local path still returns a launch spec instead of a daemon-owned PTY stream. That lets the UI and backend agree on lifecycle now while leaving a clear seam for a future transport swap.
- Treated header overrides as one-shot run overrides on paused waves instead of mutating wave config. Reviewers can verify `design` (or another override) runs once without rewriting the wave's default flow.
- Made terminal embedding additive instead of a takeover. Reviewers can inspect the native wave detail and only drop into Ghostty when an interactive run actually exists.
- Added explicit terminal-session storage/events rather than inferring state from wave status alone. The same session id now connects backend lifecycle, UI routing, and completion handling.

## How it fits together

`lfd` now creates and stores a terminal session when a paused wave is run interactively, exposes attach/start/complete/cancel endpoints for that session, and emits terminal-session updates alongside wave events. The Swift client calls those endpoints through `LocalWaveService`, stores the resulting session metadata in `RepoState`/`TerminalWorkspaceStore`, and `WaveWorkspaceView` switches between Work and Terminal tabs based on the tracked session state.

## Risks and bottlenecks

- The current terminal transport is still local-launch based: `attach` returns a wrapped shell command that reports completion back to `lfd` with `curl`. Remote/daemon-owned PTY transport is still a follow-on.
- Remote repos intentionally stay on the queue/detail path because terminal embedding currently assumes a local Ghostty launch path.
- `xcodebuild test` still fails in this headless environment when `ConcertoUITests-Runner` exits before establishing a UI test connection, so final product validation still needs a rendered macOS session.
- Terminal completion depends on the wrapped command being allowed to POST back to the local daemon. If that callback is blocked, the UI can show a stale running terminal session.

## What's not included

- Server-owned PTY streaming or multi-client terminal collaboration.
- Durable terminal scrollback/reconnect across app restarts.
- Remote terminal transport for non-local repos/executors.
- A full replacement of non-interactive wave execution with terminal sessions.

## Validation

| Suite | Result |
|-------|--------|
| `cargo fmt --check` | pass |
| `cargo clippy -- -D warnings` | pass |
| `cargo test --all` | pass |
| `uv run pytest python/tests/` | 113 passed |
| `swift test --package-path swift` | pass (10 XCTest + 246 Swift Testing cases) |
| `tests/e2e/test_smoke.sh && uv run pytest tests/e2e/test_api_smoke.py tests/e2e/test_concurrent_clients.py -v` | smoke pass + 16 passed |
| `cd swift && xcodegen generate && xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' -derivedDataPath /tmp/LoopflowSwiftGate.<ts> CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO` | app build + package/unit tests passed; `ConcertoUITests-Runner` exited early before establishing the UI test connection in this no-rendering environment |

## Manual follow-up

Run `uv run python scripts/concerto-dev.py run-debug` on a rendered macOS desktop and verify:

1. Selecting a wave opens the workspace surface instead of a terminal takeover.
2. Changing the header typeahead to `design` launches that override for a paused wave.
3. Terminal appears only while that selected wave has an active interactive run.
4. Exit status `0` resumes/completes the wave and non-zero fails it.
5. No selection still lands on the repo-wide attention queue.
