# Wave crons review

## What was implemented
- Added first-class `wave_crons` storage plus migration from legacy `mode: cron` / `waves.cron` to per-wave cron rows.
- Removed `cron` as a wave mode, allowed `workers: 0`, and changed cron dispatch to run supplementary flows independently of the wave worker budget.
- Exposed cron data through Rust, Python, and Swift models plus the HTTP API, `GET /waves/{id}/crons`, and wave serialization.
- Updated docs to teach the new `crons:` config shape and added client / contract coverage for cron payloads.

## Key choices
- **Separate `wave_crons` table, not JSON on `waves`.** Each cron entry now owns its own `last_triggered_at`, so weekly and daily schedules on the same wave do not interfere with each other.
- **Crons bypass per-wave workers but still use scheduler slots.** The primary flow keeps its own worker budget, while scheduled maintenance still respects global concurrency.
- **`workers: 0` is valid.** Cron-only governance waves no longer need a fake worker just to exist.
- **Replace-all cron updates.** Wave updates replace the wave's cron rows in one API call instead of trying to preserve mixed legacy/new representations.

## How it fits together
`wave/<name>.yaml` can now declare `crons:` entries with `flow` and `schedule`. `create_wave_handler` / `update_wave_handler` validate those entries, persist them in `wave_crons`, and `render_wave_dto` includes them in API responses. The cron poller now scans cron rows directly, decides due work per entry, starts an immediate activation with the cron's flow override, and records `last_triggered_at` back on that cron row.

## Risks and bottlenecks
- Full macOS `xcodebuild test -scheme Concerto` still flakes locally because `ConcertoUITests-Runner` exits before bootstrapping, even after the non-UI suites complete successfully.
- `replace_wave_crons` is delete-then-insert, so cron edits reset cron IDs and last-triggered timestamps by design.
- SQLite migration depends on `ALTER TABLE ... DROP COLUMN`; the repo's migration test covers the supported runtime, but older SQLite builds outside supported environments could still be a concern.

## What's not included
- Concerto UI for displaying or editing cron lists; this branch only carries cron data through shared models and services.
- Additional cron validation beyond the existing scheduler parsing logic.
- Retry / repair behavior changes for failed cron-triggered runs.

## Validation
- `cargo fmt --check`
- `cargo clippy -- -D warnings`
- `cargo test --all`
- `uv run pytest python/tests/`
- `uv run pytest python/tests/test_client.py -q`
- `uv run pytest tests/e2e/test_api_smoke.py tests/e2e/test_concurrent_clients.py -v`
- `swift test --package-path swift`
- `cd swift && xcodegen generate && xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO -only-testing:ConcertoTests`
- `cd swift && xcodegen generate && xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO` → fails locally with `ConcertoUITests-Runner ... Early unexpected exit ... signal kill before establishing connection`
