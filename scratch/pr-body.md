## Try it!
- `cargo test --all`
- `uv run pytest python/tests/`
- `uv run pytest tests/e2e/test_api_smoke.py tests/e2e/test_concurrent_clients.py -v`
- `swift test --package-path swift`
- `cd swift && xcodegen generate && xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO -only-testing:ConcertoTests`

Validation snapshot on March 27, 2026:
- Rust suite passed (`cargo test --all`)
- Python unit suite passed (`115 passed`)
- API/concurrent-client smoke passed (`16 passed`)
- Swift package tests passed
- Focused macOS app tests passed (`-only-testing:ConcertoTests`)
- Full `xcodebuild test` still flakes locally because `ConcertoUITests-Runner` exits before bootstrapping

## Intent
Replace the old single-cron wave mode with first-class cron entries that let one wave keep a primary flow and also run scheduled maintenance flows. This gives member waves weekly/monthly upkeep, lets root governance waves run on schedules with `workers: 0`, and makes cron triggering correct when a wave has more than one schedule.

## Assumptions
- Supported SQLite environments can apply the migration that drops the old `waves.cron` column.
- Cron-triggered work should ignore per-wave worker limits but still respect the global scheduler semaphore.
- Replace-all cron updates are acceptable even though they mint new cron IDs and reset `last_triggered_at` for edited cron lists.

## Key decisions
- Added a dedicated `wave_crons` table so each cron entry owns its own `last_triggered_at`.
- Removed `mode: cron` entirely instead of supporting two scheduled-work paths.
- Treated `workers: 0` as valid so cron-only waves do not need a dummy worker.
- Carried cron data through Rust, Python, and Swift models plus `GET /waves/{id}/crons` so clients can inspect schedules directly.
- Added reviewer-facing follow-up polish: Python client request coverage, Swift contract coverage, and doc cleanup for the new API shape.

## Not included
- Concerto UI for rendering/editing cron lists.
- New retry semantics for failed cron runs.
- Extra cron expression validation beyond the existing scheduler parser.
