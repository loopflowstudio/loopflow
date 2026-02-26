# Signal simplification + CI activation polish review

## What was implemented

- Unified stimulus typing around `Signal` and removed wave-run sidecar discriminants (`run_kind`, `ci_fix_kind`) from runtime/store paths.
- Added per-stimulus flow overrides (`stimulus.flow`) and wired activation dispatch to apply the override to the created run snapshot.
- Moved CI failure handling onto the activation queue model:
  - CI webhook/poll emits `ci_failure` events.
  - `triggers/ci_failure.rs` resolves/creates a `Signal::CiFailure` stimulus with `flow: ci-fix`.
  - Failures enqueue pending activations instead of direct sidecar spawning.
- Updated build flow ordering to `implement → compress → lint → gate → update-wave`.
- Updated schema and store mappings for `stimuli.signal`, `stimuli.flow`, and dropped wave-run kind columns.

## Key choices

- **Keep external API field name `kind` for stimulus DTO/config parsing** while mapping to internal `Signal`. This avoids a user-facing config break while still simplifying internals.
- **Scope CI recursion guard by flow name (`ci-fix`)** instead of a run-type enum.
- **Treat CI-fix as a normal flow run** and let activation metadata represent source/intent.
- **Polish pass additions in this gate run:**
  - Added focused CI-failure trigger tests (`resolve/reuse/create/enqueue`).
  - Renamed internal store query naming from `ListStimuliByKind` to `ListStimuliBySignal` for consistency.
  - Added explicit warning/error handling when CI failure activation enqueue fails or is dropped.

## How it fits together

Waves now run through one activation pipeline: triggers enqueue `PendingActivation`, dispatcher creates a standard `WaveRun`, and execution consumes the run regardless of source. Stimulus metadata (`signal`, optional `flow`) controls *why* the run fired and *which* flow it executes. CI failure is now just another signal source instead of a parallel executor path.

## Risks and bottlenecks

- **Flow-name guard coupling:** CI recursion prevention relies on `snapshot.flow == "ci-fix"`; renaming the flow without updating guard logic can reintroduce recursion.
- **Migration assumptions:** `017_signal_simplification.sql` assumes databases can drop columns cleanly and that existing `kind` data is valid for backfill.
- **Concurrency on CI stimulus creation:** resolution is list-then-create; current handler is serialized by one event loop task, but future parallelization would need uniqueness enforcement.

## What's not included

- No broader redesign of pre/post-step git sync or push-recovery behavior in wave execution.
- No external API rename from `stimulus.kind` to `stimulus.signal`.
- No Swift/Python/UI behavior changes beyond docs/flow metadata touched in this branch.

## Validation run

- `cargo fmt --all -- --check`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test --all`
