# Chords listen authoring + CI fix rename review

## What was implemented

- Added schema YAML support for `stimulus.kind: listen` with `source` and optional `source_repo`.
- Added repo-scoped source resolution for listen stimuli during wave creation (`source` name/ID → `source_wave_id`).
- Added executor-side listen triggering: when a source wave run completes successfully, enabled listener waves are started (or queued).
- Added shared pending activation queue/coalescing + a new drain loop that retries deferred activations.
- Applied sidecar terminology rename to CI fix terminology across executor/types/store/docs:
  - `WaveRunKind::Sidecar` → `WaveRunKind::CiFix`
  - `sidecar_kind` → `ci_fix_kind`
  - `executor/wave/sidecar.rs` → `executor/wave/ci_fix.rs`
  - migration `015_rename_sidecar_kind_to_ci_fix_kind.sql`
- Added/updated tests for listen schema parsing, listen trigger behavior, queue drain behavior, and repo-scoped source resolution.
- Updated README stimulus docs with listen YAML example.

## Key choices

- **Fail fast for listen source resolution:** listen source validation/resolution now happens before wave persistence/workspace setup, avoiding create-then-delete cleanup paths for invalid listen configs.
- **Queue over drop:** when listener is already running or scheduler slots are full, activations are queued/coalesced instead of being lost.
- **Success-only triggering:** listeners fire from completed source runs, not failed runs.
- **Terminology cleanup as migration:** column rename handled via migration instead of compatibility aliases.

## How it fits together

Wave creation parses schema stimulus definitions, and listen stimuli resolve `source` to a concrete source wave ID (repo-scoped, optional cross-repo via `source_repo`). During execution, `FlowAction::Complete` triggers a listener scan: each matching enabled listen stimulus either starts a run immediately or queues a pending activation. A background pending-activation drain loop periodically retries queued activations when waves are runnable and scheduler capacity is available.

## Risks and bottlenecks

- Pending activations are drained on a fixed interval (5s), so deferred runs are not instant.
- Listener triggering currently scans all listen stimuli then filters in memory; fine for low cardinality, but could become hot if deployments add many listen stimuli.
- Source wave must exist at listener creation time; out-of-order provisioning still errors.

## What's not included

- No context payload injection from source runs into listener prompts.
- No failure-triggered listen behavior (completed-only).
- No cycle detection for chained listen graphs.
- No multi-source listen schema support.

## Validation run

- `cargo fmt --all -- --check`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test -p loopflow`
- `cargo test -p loopflow -- lfd::http::routes::waves::tests::`
- `cargo test -p loopflow -- listen_stimulus_schema listen_trigger listen_queue`
- `uv run pytest python/tests/`
- `swift test --package-path swift`
- `tests/e2e/test_smoke.sh`
- `uv run pytest tests/e2e/test_api_smoke.py -v`
