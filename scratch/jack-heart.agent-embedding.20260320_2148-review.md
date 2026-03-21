# Review: actionable roadmap workspace

## What was implemented

Added an actionable roadmap pane to the Concerto workspace for roadmap-driven waves.

- Wave README parsing now surfaces roadmap cards, inline summaries, scratch docs, and roadmap metadata in `WaveContent`.
- Roadmap cards can reprioritize items by renaming the underlying roadmap file to the selected `1-`/`2-`/`3-`/`4-` prefix.
- The play button now starts a run against the exact roadmap file the user clicked by sending a targeted `roadmap_item` override through the Swift client, HTTP API, and wave executor.
- `lf ops ingest` now accepts an explicit roadmap item selection, so daemon-initiated runs and manual ops use the same targeted-ingest path.

## Key choices

- **Thread targeted ingest through the existing run API instead of inventing a separate endpoint.** The UI sends `roadmap_item`, the daemon passes it through activation, and ingest resolves the exact file.
- **Keep file renames as the source of truth for priority.** Reprioritization updates the actual roadmap filename rather than storing a second ordering field.
- **Make Swift roadmap ordering match Rust ingest ordering.** Canonical bucket prefixes (`1-`…`4-`) now sort ahead of legacy zero-padded filenames, so the roadmap pane matches what ingest will pick next.

## How it fits together

Concerto parses each wave README and roadmap directory into `WaveContent`, then renders those items inside the multiplexer roadmap pane. Reprioritization renames the local roadmap markdown file and reloads wave content; play sends a one-shot `roadmap_item` override to `POST /v0/waves/:id/run`, and the daemon carries that override into activation so `ops::ingest` copies that exact roadmap file into `scratch/` before the build flow runs.

## Risks and bottlenecks

- Reprioritization is still local-only; remote repos surface an error instead of attempting a rename.
- Mixed canonical (`2-foo.md`) and legacy (`02-foo.md`) filenames are still supported, but canonical files intentionally sort ahead of legacy zero-padded ones within the same bucket.
- The Xcode UI test job still does not complete cleanly in this headless session. The app and test bundles built, but the UI runner later failed to load `Concerto.debug.dylib` because macOS rejected the library under system policy.

## What's not included

- No remote roadmap editing workflow beyond surfacing server-side errors.
- No new roadmap metadata format beyond filename prefixes.
- No automated workaround for the headless macOS UI-test runner failure.

## Validation

Passed locally on March 21, 2026:

- `cargo fmt --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test --all` (1076 passed, 0 failed, 2 ignored)
- `.venv/bin/pytest python/tests/` (115 passed)
- `cargo test -p loopflow ops::ingest::tests` (12 passed)
- `swift test --package-path swift` (313 passed)
- `swift test --package-path swift --filter WaveContentParser` (8 passed)
- `swift test --package-path swift --filter Multiplexer` (22 passed)
- `tests/e2e/test_smoke.sh`
- `uv run pytest tests/e2e/test_api_smoke.py tests/e2e/test_concurrent_clients.py -v` (16 passed)

Attempted but environment-limited:

- `cd swift && xcodegen generate && xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO` — the app and UI test bundles built, then `ConcertoUITests-Runner` failed while loading `Concerto.debug.dylib` because macOS system policy denied the library in this headless session.
- `uv run python scripts/concerto-dev.py run-debug` — not run here because the environment has no rendering surface for launching the macOS app.
