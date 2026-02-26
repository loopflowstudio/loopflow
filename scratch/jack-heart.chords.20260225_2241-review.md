# Gate review: listen authoring + listener execution + CI fix rename

## What was implemented

- Added `listen` stimulus schema support for wave config files:
  - `stimulus.kind: listen`
  - required `source`
  - optional `source_repo`
- Added listen source resolution at wave creation time (name or ID -> `source_wave_id`) with repo scoping and self-target rejection.
- Changed completion behavior so source-wave completion triggers listening waves immediately when runnable.
- Added deferred activation behavior for listeners when blocked (already running or scheduler full), with queued/coalesced pending activations retried by a scheduler loop.
- Renamed sidecar terminology to CI fix terminology across executor/store/types/routes:
  - `WaveRunKind::Sidecar` -> `WaveRunKind::CiFix`
  - `sidecar_kind` -> `ci_fix_kind`
  - `executor/wave/sidecar.rs` -> `executor/wave/ci_fix.rs`
  - migration `016_rename_sidecar_kind_to_ci_fix_kind.sql`
- Updated user/docs surfaces (README, `docs/lfd.md`, and wave/chords docs) for listen + CI fix naming.

## Key choices

- **Eager validation for listen source resolution**: listener wave creation fails fast if the listen source cannot be resolved, rather than persisting partial state.
- **Start-now, queue-when-blocked activation strategy**: on source completion, listeners start immediately when possible; otherwise activation is queued/coalesced for retry.
- **Terminology migration over compatibility shim**: DB column and enums were renamed directly with a migration instead of preserving old names in code paths.
- **Main-run-only CI targets**: CI webhook targeting logic explicitly excludes CI-fix runs from being retargeted.

## How it fits together

Wave creation parses schema stimulus config, validates `listen` shape, and resolves `source/source_repo` into a persisted `source_wave_id`. At runtime, when a source wave run completes successfully, the executor scans enabled listen stimuli for that source and attempts to start listener runs immediately; blocked listeners are queued through pending activation storage. Scheduler trigger loops then dispatch queued activations when capacity/availability returns, while CI failure handling continues through the renamed CI-fix run path.

## Risks and bottlenecks

- Listener lookup on completion currently scans listen stimuli and filters in memory; large fan-out/cardinality may need indexed lookup.
- Deferred starts are eventually consistent (drain/dispatch loop driven), not instant.
- Listen source resolution is creation-order coupled: source wave must exist before creating the listener wave.
- No cycle detection for chained listen graphs yet.

## What's not included

- Source-run context injection into listener prompts (summary/diff payloads).
- Failure-triggered listen mode.
- Multi-source listen schema.
- Listen graph cycle detection and prevention.

## Validation run

- `cargo fmt --all -- --check`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test --all`
- `uv run pytest python/tests/`
- `swift test --package-path swift`
- `cd swift && xcodegen generate && xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO`
- `tests/e2e/test_smoke.sh`
- `uv run pytest tests/e2e/test_api_smoke.py -v`

All passed locally.
