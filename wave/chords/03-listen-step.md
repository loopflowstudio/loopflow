# 03: Listen Authoring

Wire listen stimuli into wave schema files and make listen triggers actually execute.

## Status

Shipped on `jack-heart.chords.20260225_2241`.

## What shipped

### Schema authoring for `listen`

- Added schema support for:

```yaml
stimulus:
  kind: listen
  source: infra
  source_repo: /Users/jack/src/other-repo # optional
```

- `source` is required for `kind: listen`
- `source_repo` is optional; defaults to the listener wave's repo
- `source` resolves to `source_wave_id` at wave creation time (name or ID)

### Eager source validation

- Listen sources are resolved before wave persistence/workspace setup
- Invalid listen config fails fast (no create-then-cleanup path)
- Self-reference is rejected
- Source wave must exist when listener wave is created

### Listen execution trigger

- On `FlowAction::Complete`, completed source runs now trigger enabled listeners
- Triggering is success-only (`Completed`, not `Failed`)
- If listener is already running or scheduler is full, activation is queued/coalesced
- A shared pending-activation drain loop retries deferred activations

### Terminology cleanup

- Renamed sidecar terminology to CI fix terminology:
  - `WaveRunKind::Sidecar` → `WaveRunKind::CiFix`
  - `sidecar_kind` → `ci_fix_kind`
  - `executor/wave/sidecar.rs` → `executor/wave/ci_fix.rs`
- Added migration `016_rename_sidecar_kind_to_ci_fix_kind.sql`
- CI webhook target updates now apply to main runs only (CI-fix runs are excluded)

### Validation

- Added/updated tests for listen schema parsing, source resolution, trigger behavior, and queue/drain behavior
- Full validation run completed across Rust, Python, Swift, and smoke suites

## Decisions locked

- Keep eager source resolution with FK-backed `source_wave_id`
- Keep success-only listen triggering
- Queue/coalesce deferred activations instead of dropping them
- ~~Keep `CiFixKind` as an enum (extensible)~~ — superseded by Phase 03.5 signal simplification (`wave/chords/035-signal-simplification.md`)

## Carry-forward follow-ups

- Context injection from source runs into listener prompts (`none | summary | full`)
- Optional failure-triggered listening behavior
- Cycle detection for chained listen graphs
- Multi-source listen schema support
- Optimize listener lookup and drain latency if listen cardinality grows
