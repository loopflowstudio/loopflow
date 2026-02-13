# Review: Docker startup durability + recovery

## What was implemented

- Added Docker-aware startup recovery to `lfd` via `WaveExecutor::recover_startup()`.
- Docker mode now rehydrates running agent containers on daemon restart instead of blanket-failing all in-flight runs.
- Added orphan cleanup for loopflow-managed containers on startup (`io.loopflow.managed=true`).
- Persisted Docker `container_id` on agents (`agents.container_id`, migration `003_agent_container_id`) and threaded it through SQLite/Postgres row mapping + writes.
- Added Docker agent labels (`io.loopflow.agent-id`, `io.loopflow.wave-id`, `io.loopflow.wave-run-id`) for recovery metadata.
- Extended tests for startup recovery behavior, including:
  - rehydrate + orphan cleanup path
  - label coverage
  - terminal run/wave protection when stale running agents are marked lost

## Key choices

- **Reattach, not resume execution state**: recovery tails running containers and finalizes the run; it does not reconstruct flow iterator state.
- **Recover before scheduler loops start**: startup reconciliation happens before new work can be scheduled, avoiding duplicate runs.
- **Dedicated `container_id` field**: kept `pid` semantics for local mode and avoided mixed-type overload.
- **Conservative status updates**: when marking lost agents, wave/run status is only flipped when the run is still non-terminal.

## How it fits together

`bin/lfd.rs` now calls `executor.recover_startup()` after store/executor init and before starting scheduler loops. In Docker mode, recovery inspects running agents, reattaches known-live containers into the in-memory active map, marks missing containers as failed, then removes unmanaged leftovers among loopflow-labeled containers. Normal execution (`DockerExecutor::run`) writes `container_id` and labels so restart recovery has durable lookup data.

## Risks and bottlenecks

- Reattached runs still cannot resume mid-flow state; successful reattach finalizes the run and future stimuli start fresh.
- Logs emitted while lfd was down are not replayed; streaming resumes only after reattach.
- Startup cleanup relies on Docker API availability; failure is logged and startup continues.

## What's not included

- Full step-by-step flow resumption across lfd restarts.
- Docker log persistence/replay across downtime.
- Volume lifecycle cleanup beyond existing wave cleanup behavior.
- Changes to user-facing CLI syntax or README examples.

## Validation

- `cargo fmt --all`
- `cargo clippy --all -- -D warnings`
- `cargo test --all`
- `uv run pytest python/tests/`
- `tests/e2e/test_smoke.sh`
