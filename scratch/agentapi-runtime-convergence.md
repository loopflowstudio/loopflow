# Runtime Convergence

## Goal
Converge provider runtime behavior so `lf` and `lfd` share one engine-owned launch path, while keeping session lifecycle concerns in `lfd`.

## Current state (after prompt convergence)
This branch landed the prompt-convergence slice:

- Shared launch-prep module in `rust/loopflow/src/engine/launch.rs`
  - `LaunchPromptInput`
  - `prepare_launch_prompt()`
  - `ContextSourceOverrides`
- `lf/commands/run.rs` now uses shared launch prep.
- `lfd/prompt.rs` now uses the same shared launch prep.
- `lfd/executor/helpers.rs` now sets run mode from `ConcreteStep.step.interactive`.
- Prompt parity tests cover engine + lfd launch-prep behavior.

What this means now:

- Prompt assembly drift is reduced.
- Provider process/runtime drift still exists (`engine/agent` vs `lfd/sessions/harness/*`).
- Wave execution still has no session-backed interactive auto-launch path.

## Decisions to keep

- No `lf -> lfd` dependency.
- Shared runtime logic belongs in `loopflow-engine`.
- `lfd` keeps session lifecycle, persistence, SSE, and API ownership.
- Interactive routing source of truth is `ConcreteStep.step.interactive`.

## Remaining work (priority order)

1. **Extract shared engine harness runtime**
   - Move provider launch/lifecycle logic into one engine-owned harness interface.
2. **Route `engine::agent::launch_agent` through that harness**
   - Keep one-shot CLI behavior via adapter, not duplicate provider launch code.
3. **Migrate `lfd` sessions to the same engine harness**
   - Keep status/replay/session metadata in `lfd`; remove duplicated provider spawning logic.
4. **Apply wave execution policy**
   - `interactive == true` => session path.
   - `interactive == false` => direct engine path.
5. **Add runtime conformance tests**
   - Equivalent inputs produce equivalent provider args/event mapping across CLI/session surfaces.

## Sequencing decision
Prioritize **engine harness extraction first**, then migrate `lfd`, then implement session-backed interactive wave routing. Routing changes before runtime convergence would cement duplicate launch paths.

## Validation baseline
Current validation used for the prompt-convergence slice:

- `cargo fmt --all -- --check`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test --all -- --skip docker_startup_rehydrates_running_agents_and_cleans_orphans --skip docker_startup_lost_agent_does_not_flip_terminal_run_wave_status`
- `cargo test -p loopflow prepare_launch_prompt`
- `cargo test -p loopflow golden_prompt`
- `uv run pytest python/tests/`
- `swift test --package-path swift`
- `tests/e2e/test_smoke.sh`
- `uv run pytest tests/e2e/test_api_smoke.py -v`
