# Gate review: jack-heart.waves-outward — validation

The branch: `lf goal <wave> --tmux` launch primitive, `Wave.repo → repos:
[RepoWork]` across Rust/Python/Swift/fixtures, Concerto reshaped into `WavesView`
(lfd-free launch/attach), old native-chat/multipane stack + stale tests trimmed,
`ux-research` loop steps, wave state reorganized into `concerto/goals/release/
website` (+ this pass seeds `reduce`). The implementation narrative is in the
diff and code; forward-looking risks were folded into wave items (see
`wave/concerto/1-embedded-terminal-build-driver.md` for the session-registry +
goal-resolution gaps, and `wave/reduce/` for the lfd/lfq collapse). This doc keeps
only the validation record.

## Validation

Passed:

- `cargo fmt --all -- --check`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test --all`
- `cargo test -p loopflow goal::tests`
- `uv run python -m pytest python/tests/` (`144 passed`)
- `uv run python scripts/check_swift_multiplatform_boundaries.py`
- `uv run python -m ruff check scripts/concerto-dev.py scripts/check_swift_multiplatform_boundaries.py`
- `uv run python -m py_compile scripts/concerto-dev.py scripts/check_swift_multiplatform_boundaries.py`
- `uv run python -m pytest tests/regression/ -v` (`4 passed`)
- `uv run python -m pytest tests/e2e/test_api_smoke.py tests/e2e/test_concurrent_clients.py -v` (`13 passed`)
- `tests/e2e/test_smoke.sh`
- `cd website && uv run python dev.py test` (`61 passed, 3 skipped`)
- `swift test --package-path swift` (`280` Swift Testing tests plus XCTest shim passed)
- `docker version && cargo test -p loopflow docker_ -- --nocapture` (`11 passed`; Docker socket-dependent cases self-skipped locally)
- `cd swift && xcodegen generate && xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' -only-testing:ConcertoTests/KeyboardRouterTests CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO` (`17 passed`)

Attempted but not green locally:

- Full non-UI Xcode suite (`-skip-testing:ConcertoUITests`) built and ran the unit
  bundle but failed on the pre-existing `KeyboardRouterTests` chord-timeout test
  after the runner delayed MainActor scheduling by ~21s. `swift test
  --package-path swift` passed the same suite, and an isolated Xcode run of
  `ConcertoTests/KeyboardRouterTests` passed (`17 passed`). The failing file is not
  in this branch's `main...HEAD` diff.
