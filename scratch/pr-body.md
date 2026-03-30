## Try it!

- `cargo fmt --check && cargo clippy -- -D warnings && cargo test --all`
- `uv run pytest python/tests/`
- `swift test --package-path swift`
- `tests/e2e/test_smoke.sh`
- `uv run pytest tests/e2e/test_api_smoke.py tests/e2e/test_concurrent_clients.py -v`

If you want to poke the feature shape directly, create a wave config with supplemental crons and inspect the returned wave JSON/UI payload for `crons`, then run concurrent ingest tests (`cargo test ops::ingest::tests::concurrent_ingest_picks_different_items`).

## Intent

This branch adds two pieces of coordination that Loopflow was missing: scheduled supplementary wave flows via first-class cron definitions, and safer concurrent backlog pickup during `ingest`. Cron support is wired end-to-end through storage, HTTP, Python, Swift, fixtures, and docs; ingest now coordinates both PM-backed claims and local file selection so multiple workers stop grabbing the same roadmap item.

## Assumptions

- Wave scheduling still uses the existing top-level `mode`, `primary_flow`, and `workers`; crons are additive, not a replacement scheduling model.
- PM providers remain the authority for remote assignment. Notion can signal claim races with HTTP 409, while other providers still use best-effort retry semantics.
- CI’s macOS environment is the source of truth for Concerto UI coverage; local `swift test` passed, but local full `xcodebuild test` still hit an early `ConcertoUITests-Runner` bootstrap crash.

## Key decisions

- Store cron entries in a dedicated `wave_crons` table and expose them in `WaveDto` rather than overloading the `waves` row.
- Trigger due crons through the existing immediate-activation path with a flow override and update `last_triggered_at` only after dispatch succeeds.
- Keep PM claim best-effort in `ingest`: try `pm_try_claim`, refresh the local mirror, then fall back to local selection if needed.
- Use a simple ingest lock plus frontmatter-based ordering/status stamping instead of adding new runtime worker-index machinery.

## Not included

- The larger scheduling redesign proposed in `wave/model/3-wave-scheduling.md`.
- A new daemon-side claim coordinator for non-PM work.
- Any broader `garden/scan` redesign beyond consuming the new scratch frontmatter.
