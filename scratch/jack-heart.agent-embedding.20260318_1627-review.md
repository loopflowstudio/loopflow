# What was implemented

This branch turns agent embedding into a full wave-workspace stack instead of a terminal overlay.

- `lfd` now persists terminal sessions, exposes them over HTTP/events, and tracks tmux-backed interactive runs as first-class runtime state.
- Concerto now opens into portfolio/attention surfaces, then drills into a workspace-first wave detail view with a persisted multiplexer layout per wave.
- The multiplexer supports terminal, markdown, diff, and launchpad panes, with tmux session cleanup, focus routing, and Ghostty keyboard handling tuned for embedded terminals.
- Loopflow’s builtin flows/steps were reorganized around `build`, `garden`, and VSM governance flows, while PM sync and wave mutation/review workflows were expanded to match the new runtime model.
- This gate pass also polished the latest macOS workspace details: automatic window tabbing is disabled, split-pane shortcuts use layout-stable keycodes, command-palette fuzzy matches are ranked, and palette actions now stay aligned with the actual shortcut catalog.

# Key choices

- **Make `terminal_sessions` the shared source of truth.** Concerto no longer treats embedded terminals as ad-hoc Swift state. The daemon owns session identity and lifecycle; the app renders and routes around that state.
- **Persist the multiplexer as layout data, not view state.** `MultiplexerLayout` and `MultiplexerStore` encode split trees per wave, normalize terminal pane config, and survive app restarts without separate sidecar models.
- **Keep terminal typing on Ghostty’s native path.** Only explicit pane-management shortcuts stay app-owned; printable input, control chords, IME commit, and paste continue to reach the terminal the right way.
- **Prefer flow taxonomy over accreted aliases.** The branch consolidates old tend/ship/build naming into clearer `build`, `garden`, `wave`, and VSM structures so daemon automation and UI affordances speak the same vocabulary.
- **Use a fresh Xcode derived-data directory for deterministic test runs.** Shared DerivedData intermittently tried to reuse UI-test build products in a bad state; isolated `-derivedDataPath` runs reliably passed `ConcertoTests` on this host.

# How it fits together

`lfd` is now the runtime spine: it stores wave/run/session state, emits attention/session/wave events, and exposes the terminal-session contract Concerto consumes. Concerto layers portfolio, attention queue, wave detail, and a persisted split-pane workspace on top of those daemon records while Ghostty/tmux provide the actual terminal surface.

On the automation side, the Rust flow engine and PM ops now assume the same run-centric model: waves define defaults and policy, while runs, triggers, repair flows, terminal sessions, and PM-linked roadmap items are all concrete execution artifacts around that shared state.

# Risks and bottlenecks

- **UI-test bootstrap is still unstable locally.** `ConcertoUITests/ScreenshotPipelineTests/testCapture` still crashes `ConcertoUITests-Runner` with `signal kill` before XCTest connects, so screenshot coverage remains blocked on the harness.
- **ConcertoTests need isolated derived data for reliability on this host.** The shared default DerivedData path produced a transient UITest bundle write failure even for `-only-testing:ConcertoTests`; a fresh `-derivedDataPath` avoided it.
- **Keyboard routing is intentionally keycode-sensitive for pane splits.** `⌃⇧5` and `⌃⇧'` are more layout-stable than character matching, but they still deserve broader non-US keyboard validation.
- **The remote story is still transitional.** Terminal sessions are durable and local-first, but daemon-owned remote PTY transport is still not part of this branch.
- **This is a wide branch.** Flow/runtime/PM/UI changes land together, so regressions may show up at the seams between daemon execution, event delivery, and Concerto presentation.

# What's not included

- A remote terminal transport or daemon-owned cross-host PTY stream.
- Final algedonic escalation routing through the full wave hierarchy.
- Stable screenshot/UI harness coverage for macOS UI tests.
- New pane types beyond terminal/markdown/diff/launchpad, richer directional focus semantics, or named saved workspace layouts.

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

Known failing validation on this host:

- `tmpdir=$(mktemp -d /tmp/loopflow-xcode-ui.XXXXXX) && cd swift && xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' -derivedDataPath "$tmpdir" CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO -only-testing:ConcertoUITests/ScreenshotPipelineTests/testCapture`
  - Fails because `ConcertoUITests-Runner` exits early with `signal kill` before XCTest establishes a connection.
