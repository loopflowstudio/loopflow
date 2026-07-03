# Gate review: jack-heart.waves-outward

## What was implemented

- Added `lf goal <wave> --tmux`, an lfd-free launch primitive that creates or reuses `../<repo>.<wave>`, starts the wave goal loop in a detached tmux session, prints the tmux handle, and stays idempotent on relaunch.
- Changed the wave wire model from one flat `repo`/status/iteration surface to `repos: [RepoWork]` across Rust, Python, Swift, and shared DTO fixtures.
- Reshaped Concerto into `WavesView`: repo rail, wave list, disk-authored wave placeholders, and embedded terminal attach through bundled `lf goal --tmux`.
- Removed the old native-chat/multipane/macOS view stack and stale tests that no longer match the "frame, don't render" Concerto direction.
- Added `ux-research` loop steps and reorganized wave state into `concerto`, `goals`, `release`, `website`, and seeded `reduce`.

Gate polish in this pass:

- Fixed Rust DTO assembly so each nested `RepoWorkDto.active_run` is selected from that repo's own runs, not from the wave's single global latest run.
- Fixed Concerto event routing so connected snapshots and wave events update every matching repo state in a multi-repo wave, not only `repos.first`.

## Key choices

- Kept `Wave::repo()` / primary-repo accessors as a temporary single-repo bridge for call sites that still need one obvious repo. Repo filters and DTO surfaces use `repos.iter()` membership.
- Session names derive from the deterministic wave worktree basename (`lf-<repo>-<wave>`), matching `lf goal --tmux` and Concerto's attach path.
- `lf goal --tmux` redirects detached tmux stdio, and the macOS launcher captures the printed handle through a temp file so GUI reads do not block forever on inherited pipes.
- Disk-authored waves are the baseline Concerto list. lfd live waves are an overlay; lfd absence should not block rendering or launching.

## How it fits together

`lf goal --tmux` owns local launch: resolve main repo, ensure sibling worktree, start or reuse the tmux session, print the handle. Concerto scans repos and disk-authored waves, merges them with any lfd waves, and calls the local launcher when a row is selected. The lfd DTO still serves live status and run/PR overlays, but execution state is nested under `repos` so repo becomes a filter rather than the wave's container.

## Risks and bottlenecks

- `lf goal --tmux` still resolves the goal from the main-derived sibling worktree. Dev-only wave dirs surfaced via `CONCERTO_DEV_WAVE_REPO` can still miss the launched worktree; tracked in `wave/concerto/1-embedded-terminal-build-driver.md` and `wave/concerto/2-lfd-owned-wave-identity.md`.
- Sessions launched by `lf goal --tmux` are not registered in lfd live session state. Tracked in `wave/reduce/2-session-registry.md`.
- Some internal call sites still use the primary repo bridge. That is a deliberate migration bridge, not the final multi-repo model.
- Xcode's hosted macOS test path is noisy in this headless environment: UI screenshot waits for a window, hosted app startup reads saved remote settings, and voice model-prep tests can time out under runner delay. `swift test --package-path swift` is the reliable Swift package signal here.

## What's not included

- `lfdb` extraction.
- `lf d` / `lf q` CLI namespace split.
- lfd executor deletion.
- subscription-backed live status for `lf goal --tmux` sessions.
- proactive Concerto worktree pre-allocation.

## Validation

Passed:

- `cargo fmt --all -- --check`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test --all` (`831 passed, 2 ignored` in the main lib suite plus integration/doc suites green)
- `cargo test -p loopflow lfd::http::routes::tests::build_wave_dto_selects_latest_run_per_repo`
- `cargo test -p loopflow goal::tests`
- `uv run python -m pytest python/tests/` (`144 passed`)
- `uv run python scripts/check_swift_multiplatform_boundaries.py`
- `uv run python -m ruff check scripts/concerto-dev.py scripts/check_swift_multiplatform_boundaries.py`
- `uv run python -m py_compile scripts/concerto-dev.py scripts/check_swift_multiplatform_boundaries.py`
- `uv run python -m pytest tests/regression/ -v` (`4 passed`)
- `tests/e2e/test_smoke.sh`
- `uv run python -m pytest tests/e2e/test_api_smoke.py tests/e2e/test_concurrent_clients.py -v` (`13 passed`)
- `cd website && uv run python dev.py test` (`61 passed, 3 skipped`)
- `swift test --package-path swift` (`281` Swift Testing tests plus XCTest shim passed)
- `swift test --package-path swift --filter PortfolioRepoStateTests` (`6 passed`)
- `docker version && cargo test -p loopflow docker_ -- --nocapture` (`11 passed`; Docker socket-dependent cases self-skipped locally)

Attempted but not green locally:

- `cd swift && xcodegen generate && xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO`
  - Failed first in `ConcertoUITests/ScreenshotPipelineTests.testCapture`: the UI runner never observed a window in the headless environment.
  - The hosted unit bundle then suffered long MainActor/app-host delays, repeated saved remote-connection attempts to `100.96.227.95:2486`, the known `KeyboardRouterTests` chord-timeout failure, and voice model-prep timeouts.
- Follow-up isolated Xcode reruns for `ConcertoTests/KeyboardRouterTests` and `ConcertoTests/PortfolioRepoStateTests` hung in Xcode test-runner cleanup and were interrupted. The same `PortfolioRepoStateTests` behavior passed under `swift test`.
