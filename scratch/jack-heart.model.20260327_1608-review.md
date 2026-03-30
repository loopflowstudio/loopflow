# Branch review: wave crons + concurrent ingest coordination

## What was implemented

- Added first-class wave crons across the daemon, storage, HTTP DTOs/routes, Python client/API, Swift models, fixtures, and docs.
- Added cron polling that turns due cron entries into immediate wave activations and records `last_triggered_at`.
- Added concurrent-ingest coordination: PM-backed waves now try to claim remotely before falling back to local selection, local ingest now serializes picks with a lock, and scratch copies get `status`, `claimed_by`, and `claimed_at` frontmatter.
- Normalized local roadmap ordering toward frontmatter-driven `priority` + `rank`, with tests for frontmatter ordering and concurrent ingest.
- Treated Notion HTTP 409 on `claim_item` as “already claimed” so concurrent workers retry the next item cleanly.

## Key choices

- **Cron as supplementary scheduling, not a new mode.** Existing `mode` stays `manual|loop`; cron definitions live beside the primary flow and run independently.
- **Store crons in their own table.** `wave_crons` keeps schedule metadata out of the core `waves` row and lets the poller list active cron work directly.
- **Best-effort PM claims.** `ingest` prefers `pm_try_claim`, but still refreshes/picks locally when PM is stale or unavailable.
- **Filesystem locking for local ingest.** The implementation chose the simpler lock-based fallback rather than depending on a new worker-index runtime contract.
- **Local status cache only.** `scratch/` frontmatter reflects claim state for downstream tooling, but PM remains the authority.

## How it fits together

- Wave creation/update reads cron definitions from YAML or API payloads, persists them in `wave_crons`, and exposes them through `WaveDto`, the Python client, and Swift decoding.
- The cron poller wakes every 30 seconds, lists active cron rows, checks each schedule against `last_triggered_at`, and dispatches an immediate activation with a flow override when due.
- `ingest` now claims PM items before pulling, sorts local work by normalized roadmap metadata, copies the selected item into `scratch/`, removes it from `wave/`, and stamps the scratch document as in progress.

## Risks and bottlenecks

- The cron poller uses a 30-second tick plus a 24-hour grace window. If the daemon is down longer than a day, old missed cron windows are intentionally ignored.
- PM claim semantics are still provider-limited. Notion gives a real 409 conflict, but Linear/Asana remain last-write-wins and rely on retry/review safeguards.
- Full macOS `xcodebuild test` is still the main validation risk locally: after a clean DerivedData wipe, `ConcertoUITests-Runner` exited before bootstrap even though `swift test --package-path swift` passed.

## What's not included

- The larger scheduling redesign in `wave/model/3-wave-scheduling.md` (removing top-level `mode`/`flow`/`workers`) is not implemented here.
- No lfd-side distributed claim coordinator was added; concurrency stays provider-native for PM and lock-based for local waves.
- No `garden/scan` behavior changes beyond consuming the new frontmatter written by ingest.

## Validation

### Passed

- `cargo fmt --check`
- `cargo clippy -- -D warnings`
- `cargo test --all`
- `uv run pytest python/tests/`
- `swift test --package-path swift`
- `tests/e2e/test_smoke.sh`
- `uv run pytest tests/e2e/test_api_smoke.py tests/e2e/test_concurrent_clients.py -v`

### Needs CI confirmation

- `cd swift && xcodegen generate && xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO`
  - Local result after wiping DerivedData: build/test run ended with `ConcertoUITests-Runner ... Early unexpected exit, operation never finished bootstrapping`.
