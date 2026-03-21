## Try it!

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all
.venv/bin/pytest python/tests/
cargo test -p loopflow ops::ingest::tests
swift test --package-path swift
swift test --package-path swift --filter WaveContentParser
swift test --package-path swift --filter Multiplexer
tests/e2e/test_smoke.sh
uv run pytest tests/e2e/test_api_smoke.py tests/e2e/test_concurrent_clients.py -v
uv run python scripts/concerto-dev.py run-debug
```

Then in Concerto:

1. Open a wave with roadmap items.
2. In the **Roadmap** pane, confirm each card shows a short inline summary.
3. Change a card's priority and verify the file is renamed to the new `1-`/`2-`/`3-`/`4-` prefix and the list reorders.
4. Click the play button on a planned item and verify that exact roadmap file is ingested into `scratch/` and the wave starts running.

Validation on March 21, 2026:

- `cargo fmt --check` ✅
- `cargo clippy --all-targets --all-features -- -D warnings` ✅
- `cargo test --all` ✅ (1076 passed, 0 failed, 2 ignored)
- `.venv/bin/pytest python/tests/` ✅ (115 passed)
- `cargo test -p loopflow ops::ingest::tests` ✅ (12 passed)
- `swift test --package-path swift` ✅ (313 passed)
- `swift test --package-path swift --filter WaveContentParser` ✅ (8 passed)
- `swift test --package-path swift --filter Multiplexer` ✅ (22 passed)
- `tests/e2e/test_smoke.sh` ✅
- `uv run pytest tests/e2e/test_api_smoke.py tests/e2e/test_concurrent_clients.py -v` ✅ (16 passed)

I also attempted the macOS UI test job:

- `cd swift && xcodegen generate && xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO`

That built the app and UI test bundles, but `ConcertoUITests-Runner` later failed while loading `Concerto.debug.dylib` because macOS system policy denied the library in this headless session.

## Intent

Make roadmap-driven waves usable from the Concerto workspace instead of treating roadmap files as passive documentation. The branch surfaces roadmap items as actionable cards, lets users reprioritize them by renaming the source files, and makes the play button run the exact roadmap item the user selected.

## Assumptions

- Roadmap priority remains encoded in filenames, with canonical `1-`/`2-`/`3-`/`4-` prefixes as the preferred format.
- `ship-roadmap` waves should still route through ingest before build, even when the run is targeted at a specific roadmap file.
- Local repos may rename roadmap files directly; remote repos should surface an error instead of pretending reprioritization succeeded.

## Key decisions

- Reused the existing wave run endpoint with a one-shot `roadmap_item` override rather than adding a separate ingest RPC.
- Kept file renames as the only priority source of truth, so UI state stays aligned with the underlying roadmap files.
- Aligned Swift roadmap ordering with Rust ingest ordering so canonical bucketed items sort ahead of legacy zero-padded items in both the UI and the backend.

## Not included

- No remote roadmap reprioritization flow.
- No new roadmap metadata format beyond filename prefixes.
- No automated workaround for the headless macOS UI-test runner failure.
