## Try it!

```bash
uv run python scripts/concerto-dev.py run-debug
```

Open a local repo with two paused waves and verify:

- selecting a wave opens its workspace surface instead of taking over the whole window with a terminal
- Work stays the default tab for the selected wave
- changing the header typeahead to `design` launches that override for a paused wave
- a Ghostty-backed Terminal tab appears only when that wave has an active tracked terminal session, and a freshly created session auto-opens that tab once
- exiting the terminal with status `0` resumes the wave in `lfd`; non-zero fails it
- with no selected wave, the repo window still lands on the live attention queue

Validation commands:

```bash
cargo fmt --check
cargo clippy -- -D warnings
cargo test --all
uv run pytest python/tests/
swift test --package-path swift
tests/e2e/test_smoke.sh && uv run pytest tests/e2e/test_api_smoke.py tests/e2e/test_concurrent_clients.py -v
cd swift && xcodegen generate && xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' -derivedDataPath /tmp/LoopflowSwiftGateUnit.$(date +%s) -only-testing:ConcertoTests -skip-testing:ConcertoUITests CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO
```

Validation results:

- `cargo fmt --check` ✅
- `cargo clippy -- -D warnings` ✅
- `cargo test --all` ✅
- `uv run pytest python/tests/` ✅ (115 passed)
- `swift test --package-path swift` ✅ (10 XCTest + 265 Swift Testing cases)
- `tests/e2e/test_smoke.sh && uv run pytest tests/e2e/test_api_smoke.py tests/e2e/test_concurrent_clients.py -v` ✅ (smoke pass + 16 passed)
- targeted `xcodebuild test` for `ConcertoTests` ✅
- full `xcodebuild test ...` built and launched the UI runner, but `ConcertoUITests-Runner` hung in this no-render environment before completion

## Intent

Keep Concerto's repo window workspace-first while making tracked terminal sessions sturdy enough to trust. This branch gives `lfd` first-class terminal-session persistence, uses that shared state to drive the queue/workspace/detail UI, and hardens the bundled-daemon lifecycle so local interactive runs do not leak stale helpers or lose their flow/run context.

## Assumptions

- Local repos continue to use the bundled or local `lfd` path; remote repos still stop at queue/detail surfaces until remote terminal transport exists.
- The current interactive model is still launch-spec based: `lfd` is the source of truth for terminal-session state, but it is not yet hosting daemon-owned PTYs.
- Bundled cleanup is best-effort and environment-sensitive: `pkill` is available on macOS hosts, and Docker cleanup only matters when the bundled container path is in use.

## Key decisions

- Preserve Work as the default tab and only auto-open Terminal once when a new tracked session appears.
- Show the active run flow in wave rows/workspaces so one-shot overrides like `design` on a `ship-roadmap` wave stay legible.
- Tag bundled daemon/container launches and reap stale instances on the next startup rather than waiting for manual cleanup.
- Split the deeper runtime reframe into `wave/lfd/` docs instead of pretending this branch already ships daemon-aware `lf` or daemon-hosted PTYs.

## Not included

- Daemon-owned PTY transport and attach/read-write/resize APIs
- `lf` daemon-awareness and structured lifecycle reporting back to `lfd`
- Replacing the daemon executor with real `lf <flow-or-step>` process supervision
- Remote terminal embedding
- A complete local pass of `ConcertoUITests` in this no-render environment
