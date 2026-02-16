# Wave Executor Phase 2: Split `wave.rs` by concern + unify agent launch lifecycle

## Why this exists

`rust/loopflow/src/lfd/executor/wave.rs` is still a large mixed-responsibility module (~1.2k lines). It currently combines:

- main run orchestration (`execute`, step advancement, run status updates)
- fork orchestration (fork setup, parallel branch execution, cleanup)
- CI sidecar orchestration (debug run creation, execution, git push)
- summary orchestration (freshness checks + internal summarize run)
- duplicated agent lifecycle logic (start agent, emit started event, run, end agent, emit ended event)

We already restored the top-level executor split (`mod/docker/local/helpers/wave`). This doc scopes the next reduction inside `wave`.

## Goals

1. Split `wave.rs` by concern so each area is readable and testable.
2. Introduce one shared launch lifecycle path used by:
   - `run_step`
   - fork branch execution
   - internal summarize run
   - CI sidecar debug run
3. Preserve behavior and storage/event semantics.

## Non-goals

- No schema changes.
- No behavior changes to scheduler semantics, fork policy, or CI fix flow.
- No Docker fork support in this pass.

## Current duplication to remove

The following path is repeated with minor variations:

1. build prompt/model/launch
2. create `Agent`
3. `store.start_agent`
4. emit `Event::agent_started`
5. `runner.run(...)`
6. map exit code to `AgentStatus`
7. `store.end_agent`
8. emit `Event::agent_ended`

This pattern appears in:

- `run_step`
- `run_internal_summarize`
- `execute_ci_fix_agent`
- fork branch closure inside `run_fork`

## Proposed module layout

Convert `executor/wave.rs` into `executor/wave/`:

- `executor/wave/mod.rs`
  - `WaveExecutor` struct
  - constructor/accessors (`new`, `with_runner`, `executor_type`)
  - top-level `execute`
  - run status helpers (`set_wave_status`, `fail_run`)
  - module wiring/re-exports

- `executor/wave/launch.rs`
  - shared launch lifecycle API
  - `AgentLaunchRequest`
  - `AgentLaunchOutcome`
  - `WaveExecutor::launch_agent(...)`

- `executor/wave/fork.rs`
  - `run_fork`, `run_choose`, `cleanup_fork`
  - fork-specific structs/helpers only
  - calls `launch_agent` for each branch

- `executor/wave/sidecar.rs`
  - `spawn_ci_fix_agent`
  - `run_ci_fix_agent_with_slot`
  - `execute_ci_fix_agent`
  - calls `launch_agent`

- `executor/wave/summary.rs`
  - `ensure_summary_fresh`
  - `run_internal_summarize`
  - calls `launch_agent`

Optional: keep janitor and ephemeral collection in `mod.rs` first; split later if needed.

## Core API design

### `AgentLaunchRequest`

A single request object passed to the launcher.

```rust
struct AgentLaunchRequest {
    wave_id: LfdId,
    wave_run_id: LfdId,
    repo: String,
    worktree: String,
    step: ConcreteStep,
    model: String,
    cmd: Vec<String>,
}
```

### `AgentLaunchOutcome`

```rust
struct AgentLaunchOutcome {
    agent_id: LfdId,
    exit_code: i32,
    status: AgentStatus,
}
```

### `WaveExecutor::launch_agent`

Responsibilities:

- build/store `Agent`
- start persistence + start event
- run command via `runner`
- end persistence + end event
- return normalized outcome

This becomes the only place that directly performs start/run/end bookkeeping.

## Behavior invariants to preserve

- Agent records are still created for all run types.
- Start/end events still fire exactly once per launched agent.
- `exit_code == 0` maps to `Completed`, otherwise `Failed`.
- Existing run/fork/sidecar state transitions remain unchanged.
- Summary failures still degrade gracefully (warn + continue).

## Alternatives considered

1. **Extract only modules, keep duplicated launch logic**
   - Lower risk, but leaves main complexity untouched.

2. **Trait-based launch abstraction (`LaunchBackend`)**
   - Overkill for current scope; adds indirection without immediate value.

3. **Move launch lifecycle to `helpers.rs` free function**
   - Avoids new files but weakens ownership boundaries; lifecycle is executor behavior, not generic helper behavior.

Chosen: `wave/launch.rs` method on `WaveExecutor`.

## Implementation plan

1. **Create `executor/wave/` module tree** with `mod.rs`, `fork.rs`, `sidecar.rs`, `summary.rs`, `launch.rs`.
2. **Move code by concern** without logic edits.
3. **Introduce `AgentLaunchRequest/Outcome` + `launch_agent`**.
4. **Replace duplicated launch blocks** in step/fork/summary/sidecar with calls to `launch_agent`.
5. **Keep tests green** by moving existing `wave` tests and adding focused lifecycle tests.

## Testing plan

- Existing:
  - `cargo test -p loopflow lfd::executor`
- Add/adjust:
  1. launcher success path writes `start_agent`/`end_agent` and emits start/end events.
  2. launcher non-zero exit maps to `AgentStatus::Failed`.
  3. fork branch path still marks `ForkRunStatus` correctly.
  4. summary path still stores summary only on success.
  5. CI sidecar path still commits/pushes only after successful debug run.

## Rollout and risk

Primary risk: subtle behavior drift in event ordering or status writes.
Mitigation: keep refactor mechanical, add launcher-focused tests before deleting old paths.

## Done criteria

- `wave.rs` replaced by `wave/` concern modules.
- No duplicated start/run/end lifecycle blocks remain in wave orchestration.
- `cargo check`, `cargo test -p loopflow lfd::executor`, `cargo clippy -- -D warnings` all pass.
- No externally visible behavior changes.
