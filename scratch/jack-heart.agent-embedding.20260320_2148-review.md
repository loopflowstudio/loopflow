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
- **Make Swift roadmap ordering match Rust ingest ordering.** Canonical bucket prefixes (`1-`…`4-`) now sort ahead of legacy numbered items, so the roadmap pane matches what ingest will pick next.

## How it fits together

Concerto parses wave README and roadmap files into `WaveContent`, then renders those items inside the multiplexer roadmap pane. When the user reprioritizes an item, Concerto renames the roadmap markdown file locally and reloads wave content. When the user presses play, Concerto calls `POST /v0/waves/:id/run` with a one-shot `roadmap_item` override; the daemon carries that override into activation, and `ops::ingest` copies that exact roadmap file into `scratch/` before the build flow runs.

## Risks and bottlenecks

- Reprioritization is currently local-only; remote repos cannot rename roadmap files from Concerto.
- Mixed legacy (`02-foo.md`) and canonical (`2-foo.md`) filenames are still supported, but ordering semantics now intentionally favor canonical buckets first; reviewers should validate that this matches the intended migration behavior.
- Full Xcode UI automation still depends on a live desktop session. In this headless environment, the app built and unit tests passed, but the UI-test runner crashed before finishing bootstrap.

## What's not included

- No remote roadmap editing workflow beyond surfacing server-side errors.
- No additional persistence layer for roadmap priorities beyond filename prefixes.
- No change to roadmap document format beyond parsing and file selection.

## Validation

Passed locally on March 21, 2026:

- `cargo fmt --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test --all` (998 passed, 0 failed, 2 ignored)
- `.venv/bin/pytest python/tests/` (115 passed)
- `cargo test -p loopflow ops::ingest::tests` (12 passed)
- `swift test --package-path swift` (312 passed)
- `swift test --package-path swift --filter WaveContentParser` (7 passed)
- `swift test --package-path swift --filter Multiplexer` (22 passed)
- `tests/e2e/test_smoke.sh`
- `uv run pytest tests/e2e/test_api_smoke.py tests/e2e/test_concurrent_clients.py -v` (16 passed)

Attempted but environment-limited:

- `cd swift && xcodegen generate && xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO` — the app and test bundles built, unit tests ran, then `ConcertoUITests-Runner` exited early before establishing the UI-test connection in this headless session.
- `uv run python scripts/concerto-dev.py run-debug` — not run here because the environment has no rendering surface for launching the macOS app.
