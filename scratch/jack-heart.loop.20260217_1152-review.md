# Review: foundations + live PR state follow-through

## What was implemented

- Added regression coverage in `rust/loopflow/src/lfd/http/routes/mod.rs` proving wave projections use live PR state (not snapshot state) for `open_pr_count`, and that open/merged/closed/unknown transitions are reflected in projections.
- Added store-level transition coverage in `rust/loopflow/src/lfd/store/mod.rs` for `find_next_unmerged_run` so queue head selection advances only after merged state and keeps closed/unknown runs pending.
- Hardened migration `005_wave_run_lineage_live_pr_state.sql` so historical backfill does not overwrite explicit lineage already written by newer code (`parent_run_id` and derived `stack_position` now preserve existing explicit values).

## Key choices

- **Preserve explicit lineage in migration**: `COALESCE`/`CASE` guards were added so inferred backfill only applies when `parent_run_id` is missing.
  - Alternative rejected: unconditional recomputation from ordered runs, because it can clobber authoritative lineage recorded at run creation.
- **Test live-state behavior at two layers**: route projection tests validate API-facing behavior, while store tests validate queue-selection semantics.
  - Alternative rejected: testing only one layer, which would miss regressions where store logic and DTO projection diverge.
- **Keep stale visibility explicit**: tests assert stale behavior remains visible when GitHub config lacks credentials.

## How it fits together

The migration ensures lineage fields remain trustworthy as the durable foundation. Store helpers (`find_next_unmerged_run`) consume those lineage/live-state records to pick queue head deterministically. Route projection then maps live PR truth into wave DTOs so API consumers see current open counts and stale-state signals without mutating historical snapshots.

## Risks and bottlenecks

- Migration logic depends on `run_kind = 1` representing main runs; enum/storage drift would silently affect backfill behavior.
- Route tests currently use temp sqlite files under OS temp dir; failures mid-test can leave files behind (low functional risk, mild hygiene risk).
- Stale-state behavior is intentionally conservative; deployments without GitHub credentials will consistently report stale flags, which can look noisy if not expected.

## What's not included

- No runtime behavior change to GitHub sync triggers or polling cadence.
- No API shape changes beyond behavior already present (this diff adds regression protection, not new DTO fields).
- No queue UX/Concerto presentation updates.

## Validation run

- `cargo fmt --all -- --check`
- `cargo clippy --all -- -D warnings`
- `cargo test --all`
- `uv run pytest python/tests/`
- `swift test --package-path swift`
- `tests/e2e/test_smoke.sh`
- `cd swift && xcodegen generate && xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS'`
