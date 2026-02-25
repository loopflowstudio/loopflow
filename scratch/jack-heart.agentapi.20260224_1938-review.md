# Runtime convergence review (gate)

## What was implemented

- Added a shared launch-prep module at `rust/loopflow/src/engine/launch.rs` with:
  - `LaunchPromptInput` as canonical input
  - `prepare_launch_prompt()` as shared context + `LaunchConfig` builder
  - `ContextSourceOverrides` for caller-specific source toggles
- Rewired CLI run path (`lf/commands/run.rs`) to use `prepare_launch_prompt()` instead of duplicating prompt assembly.
- Rewired lfd prompt path (`lfd/prompt.rs`) to use the same shared builder.
- Updated wave step prompt helper (`lfd/executor/helpers.rs`) to pass `run_mode` from `step.interactive` (`interactive` vs `auto`).
- Exported launch-prep types/functions from `engine/mod.rs`.
- Added/kept parity-focused tests:
  - engine launch prep model precedence + direction merge + config area behavior
  - lfd prompt prep parity against engine launch prep

## Key choices

- **Converge prompt assembly in engine**: callers still own logging/output/session concerns, but context construction and launch config generation are now single-source.
- **Caller-controlled config inclusion**: `include_config_directions` and `include_config_area` preserve prior surface-specific behavior while sharing implementation.
- **Explicit source overrides**: CLI can override lfdocs/diff/clipboard without forking prompt logic.
- **Run mode from step metadata**: wave helper now uses `ConcreteStep.step.interactive` when preparing prompts.

## How it fits together

Both `lf run` and `lfd` now feed surface-specific inputs into `engine::prepare_launch_prompt()`. That function gathers context, trims it, formats prompt/system/task sections, resolves model precedence, and returns a canonical `LaunchConfig` plus prompt metadata. CLI uses that output for terminal launch/logging; lfd uses it for session/wave execution prep.

## Risks and bottlenecks

- Provider-process runtime is still split (`engine/agent` vs `lfd/sessions/harness/*`), so this change reduces prompt drift but does not yet remove launch-path drift.
- Interactive wave routing is still mixed with existing wave executor behavior; this change only aligns run-mode prompt metadata.
- Full local `cargo test --all` includes two Docker-socket-dependent tests that fail without `/var/run/docker.sock`; validated suite was run with those two tests explicitly skipped.

## What's not included

- No harness extraction/unification across engine and lfd session runtime yet.
- No `lf`→session handoff changes.
- No session API/event model changes.
- No wave executor control-flow rewrite for session-backed interactive steps.

## Validation

- `cargo fmt --all -- --check`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test --all -- --skip docker_startup_rehydrates_running_agents_and_cleans_orphans --skip docker_startup_lost_agent_does_not_flip_terminal_run_wave_status`
- `cargo test -p loopflow prepare_launch_prompt`
- `cargo test -p loopflow golden_prompt`
- `uv run pytest python/tests/`
- `swift test --package-path swift`
- `tests/e2e/test_smoke.sh`
- `uv run pytest tests/e2e/test_api_smoke.py -v`
