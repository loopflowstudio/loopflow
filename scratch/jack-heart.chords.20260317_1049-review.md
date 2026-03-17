# Branch Review — jack-heart.chords.20260317_1049

## What was implemented

- Renamed the built-in `or` branches for roadmap and tend flows so the public flow language matches the intent of the branch:
  - `ship-roadmap` now routes to `play` or `silence`
  - `tend` now routes to `tune` or `silence`
- Replaced the old helper subflow names with `ship-roadmap-play` and `tend-tune`, and removed the deleted `ship-roadmap-build` / `tend-chord` files.
- Updated README flow tables and routing examples to match the new names.
- Updated flow tests to assert the new routing structure.
- Tightened `lf ops land` so remote land without an open PR fails fast before trying to generate PR copy.
- Hardened Rust tests against machine-local config leakage by isolating `HOME` in config/ops integration tests.

## Key choices

- **Use action words instead of implementation words.** `play` and `tune` describe the user-visible decision being made; `build`, `chord`, and `reorg` described historical implementation details.
- **Keep silence as the only no-op branch.** The old `reorg` branch made both flows do extra work even when the router's answer was effectively “not now.” This change makes quiet exits explicit.
- **Fail before generating PR copy when landing can't proceed.** `lf ops land` now checks for an open PR before invoking PR-copy generation when `--create-pr` is absent, so the user gets the right error and tests do not depend on local agent config.
- **Isolate tests from the developer machine.** Integration tests that mock `gh`/`claude` now also blank `HOME`, so `~/.lf/config.yaml` cannot silently swap in a different harness.

## How it fits together

Flow YAML is the source of truth for routing names. The README mirrors those names, and `flow_tests` verifies expansion so the docs, built-ins, and parser stay aligned. On the ops side, `land` resolves PR copy only after it knows a remote land can proceed, while the shared test env guard neutralizes host-specific config so the mocked CLI paths stay deterministic.

## Risks and bottlenecks

- Any unpublished docs, local scripts, or muscle memory that still refer to `tend-chord` / `ship-roadmap-build` will need to switch to the new names.
- `lf ops land` now changes the order of remote-land checks; the behavior is better, but it is still worth watching for regressions around PR creation/update.
- `xcodebuild test -project swift/LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO` did **not** complete locally on March 17, 2026. The app built, but the macOS UI runner timed out during test bootstrap and reported `Authentication canceled. Canceled by user.` Result bundle: `/Users/jack/Library/Developer/Xcode/DerivedData/LoopflowSwift-eqprygslnxxpslgzzmrapcqkngvp/Logs/Test/Test-Concerto-2026.03.17_14-56-27--0700.xcresult`.

## What's not included

- No new router logic for `tend/assess` or `ingest`; this is a naming and flow-shape cleanup, not a change to how decisions are made.
- No live lfd tend-cycle validation beyond the existing automated test coverage.
- No changes to the draft/review/apply chord step prompts or to `update-wave`/`reorg` semantics outside these renamed routes.

## Validation

Passed:
- `cargo fmt --check`
- `cargo clippy -- -D warnings`
- `cargo test -p loopflow --test flow_tests`
- `cargo test -p loopflow lf::commands::flow::tests`
- `cargo test -p loopflow --test config_tests`
- `cargo test -p loopflow --test land_tests`
- `cargo test -p loopflow --test pr_tests`
- `cargo test --all`
- `uv run pytest python/tests/`
- `swift test --package-path swift`
- `tests/e2e/test_smoke.sh`
- `uv run pytest tests/e2e/test_api_smoke.py tests/e2e/test_concurrent_clients.py -v`

Attempted but environment-blocked locally:
- `cd swift && xcodegen generate && xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO`
