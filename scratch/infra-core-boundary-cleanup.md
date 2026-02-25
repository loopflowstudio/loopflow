# 01: Core Boundary Cleanup

Deconcentrate hotspot files by cleaning core boundaries first (`store`, `docker`, provider command wiring).

## Why this phase exists

Current risk is concentrated responsibility in a small set of files:

- `lfd/store/mod.rs` carries a large forwarding façade plus backend-dispatch glue.
- `lfd/executor/docker.rs` mixes image lifecycle, workspace lifecycle, recovery, and container IO.
- `engine/agent.rs` uses central switch-based provider command wiring.

Before adding more capability, reduce blast radius and make boundaries explicit.

## Scope

### In scope

1. **Store boundary cleanup**
   - Reduce or remove `impl Store` forwarding boilerplate.
   - Unify session storage dispatch pattern with the rest of the store surface.
   - Keep call sites clear and migration-safe.
2. **Docker executor decomposition**
   - Split `docker.rs` into focused modules by lifecycle boundary:
     - image lifecycle
     - workspace lifecycle
     - recovery/reattach
     - container IO
   - Keep `AgentExecutor` behavior and external contract unchanged.
3. **Provider command registry**
   - Replace central switch-style command construction with provider-owned builders.
   - New provider support should require adding a provider module, not editing central match logic.

### Out of scope

- Prompt budgeting algorithm changes
- Trigger model changes (polling/webhooks)
- Flow language changes
- Session API behavior changes

## Contract

- Existing user-facing behavior remains stable (CLI flags, wave execution outcomes, session behavior).
- Boundary changes are structural, not semantic.
- Provider command behavior remains parity-checked per provider.

## Learned from direction taxonomy

Structural renames have higher stale-reference blast radius than expected. The direction restructuring required three gate passes to catch all stale references (docs, Swift previews, wave configs, test fixtures). Plan a sweep pass after each decomposition step — grep for old module paths and type names across the full repo, not just Rust.

## Validation

- `cargo fmt --all -- --check`
- `cargo clippy -p loopflow --all-targets -- -D warnings`
- `cargo test -p loopflow store_`
- `cargo test -p loopflow docker_`
- `cargo test -p loopflow build_model_command`

## Done when

- `store` surface no longer pays heavy forwarding tax for normal call paths.
- `docker` executor logic is decomposed into lifecycle-focused modules with clear ownership.
- Provider command construction is trait/registry-based instead of central branching.
- Existing behavior is preserved with passing tests and no contract regressions.
