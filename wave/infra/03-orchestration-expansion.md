# 03: Orchestration Expansion

Expand trigger and flow capabilities after core boundaries and contracts are stable.

## Why this phase exists

Push responsiveness and richer orchestration are high leverage, but they should land on stable seams.

With boundary cleanup + contract hardening complete, this phase can expand behavior without compounding fragility.

## Scope

### In scope

1. **Push-based stimuli**
   - Add webhook/file-watch style activation paths alongside polling.
   - Keep polling as fallback and safety net.
2. **Flow system enrichment**
   - Conditional flow behavior where explicit and testable.
   - Richer fork fan-out/composition patterns.
3. **Operational safeguards**
   - Clear observability for trigger source, activation path, and flow branch decisions.
   - Backpressure/queue-safe behavior under high event rates.

### Out of scope

- Replacing scheduler model wholesale
- Studio auth/hosting roadmap items from other waves
- Broad UI redesign in Concerto
- Reworking core persistence model

## Contract

- Existing loop/watch/cron behavior remains supported.
- New trigger paths are additive and debuggable (source and reason are visible).
- Flow behavior remains deterministic and replayable from stored run context.

## Validation

- `cargo fmt --all -- --check`
- `cargo clippy -p loopflow --all-targets -- -D warnings`
- `cargo test -p loopflow triggers`
- `cargo test -p loopflow flow`
- `tests/e2e/test_smoke.sh`

## Done when

- Waves can react through push paths with polling fallback.
- Flow composition supports richer patterns without ambiguous execution.
- Trigger/flow behavior is observable and diagnosable in normal ops workflows.
