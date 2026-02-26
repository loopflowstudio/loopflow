# 03.5: Signal Simplification + Parallel Execution

Unify CI fix and listen under one stimulus activation model. Add parallel execution for non-serialized waves.

## Status

Shipped on `jack-heart.chords.20260225_2241`.

## What shipped

### Signal simplification

- Replaced internal `StimulusKind` usage with `Signal`
- Added `stimulus.flow` override so a stimulus can select a flow at activation time
- Removed `WaveRunKind` and `CiFixKind` from runtime and storage paths
- Removed `executor/wave/sidecar.rs` and folded CI fix execution into normal wave execution
- Moved CI failure handling onto pending activations:
  - CI webhook/poll emits `ci_failure` events
  - Trigger logic resolves/creates a `Signal::CiFailure` stimulus with `flow: ci-fix`
  - Failures enqueue activations instead of directly spawning a sidecar run
- Updated build flow ordering to `implement → compress → lint → gate → update-wave`
- Added migration `017_signal_simplification.sql` for `stimuli.signal`, `stimuli.flow`, and wave-run kind column removal

### Parallel execution foundation

- Added `wave.serialized: bool` to control queue vs parallel dispatch
- Non-serialized waves spawn per-run worktrees with `-run-{hash}` suffix
- Serialized waves continue using the pending activation queue for sequential dispatch
- Added pre-step `fetch+rebase` and post-step `commit+push` git sync at step boundaries
- Added migration `018_wave_serialized.sql` for the serialized flag

## Decisions locked

- Keep external API/config field name `kind` for stimulus DTO parsing; map it to internal `Signal`
- Keep CI recursion guard keyed by flow name (`ci-fix`) for now
- Treat CI fix as a normal flow run; activation metadata captures source/intent

## Not included in this phase

- Git sync recovery paths (rebase conflict handling, push failure escalation) — deferred to Phase 03.6
- Dual rebase (wave-branch + default-branch) — deferred to Phase 03.6
- External API rename from `stimulus.kind` to `stimulus.signal` — coordinated rollout needed
- Swift/Python/UI expansion beyond docs and flow metadata touched here

## Carry-forward risks

- CI recursion guard is coupled to the literal flow name `ci-fix`
- Migration 017 assumes clean backfill from legacy stimulus `kind` values
- CI stimulus resolution is list-then-create and relies on current single-task serialization

## Validation

- `cargo fmt --all -- --check`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test --all`
