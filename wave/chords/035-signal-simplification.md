# 03.5: Signal Simplification

Unify CI fix and listen under one stimulus activation model.

## Status

Shipped on `jack-heart.chords.20260225_2241`.

## What shipped

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

## Decisions locked

- Keep external API/config field name `kind` for stimulus DTO parsing; map it to internal `Signal`
- Keep CI recursion guard keyed by flow name (`ci-fix`) for now
- Treat CI fix as a normal flow run; activation metadata captures source/intent

## Not included in this phase

- Pre/post-step git sync hardening and push recovery changes across wave execution
- External API rename from `stimulus.kind` to `stimulus.signal`
- Swift/Python/UI expansion beyond docs and flow metadata touched here

## Carry-forward risks

- CI recursion guard is coupled to the literal flow name `ci-fix`
- Migration 017 assumes clean backfill from legacy stimulus `kind` values
- CI stimulus resolution is list-then-create and relies on current single-task serialization

## Validation

- `cargo fmt --all -- --check`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test --all`
