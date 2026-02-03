# Design Review: lfd as Primary Execution Path

Branch: `jack-heart.rust.20260203_1015`

## What was implemented

**lfd is now the primary execution path for flows.** This branch:

1. Added `WaveRun` as the execution record, separating execution state from wave config
2. Introduced `WaveExecutor` in lfd to run steps, fork branches, and stream output
3. Removed `runtime.rs`, `store.rs`, and `lf-engine` binary from loopflow-engine
4. Rewired all triggers (loop/watch/cron/RunWave/ConnectWave/EndAgent) to use `WaveExecutor`
5. Added `expand_flow()` and `next_action()` to loopflow-engine for flow expansion with `flow_parents` tracking
6. Consolidated `Choose` into `Fork` with `ForkSelect` modes (all/one/prompt)
7. Added `FlowRef` variant for nested flow references

## Key choices

| Decision | Rationale |
|----------|-----------|
| Execution state on `WaveRun`, not `Wave` | Wave is config; WaveRun is execution instance. Supports run history, concurrent runs in future. |
| `ForkSelect` enum merges Choose into Fork | Simpler model: one construct for branching with selection mode. |
| Loop ticker acquires scheduler slots before creating WaveRun | Prevents stuck "running" runs if scheduler is full. |
| `flow_parents` stored per-run, derived from step_index | Enables commit messages like `lf grind ship implement: ...` without re-parsing flow. |
| OutputHub uses broadcast channel | Supports multiple StreamOutput subscribers per wave_run. |

## How it fits together

```
Trigger (loop/watch/cron/RunWave)
    │
    ▼
create_wave_run() → WaveRun(Pending)
    │
    ▼
WaveExecutor.execute(run_id)
    │
    ├─► next_action(plan, step_index)
    │       │
    │       ├─► RunStep → spawn agent → stream output → update Agent/WaveRun
    │       ├─► WaitInteractive → create Agent(Waiting), WaveRun(Waiting), return
    │       ├─► Fork → create worktrees, spawn parallel branches, synthesize, cleanup
    │       └─► Complete → WaveRun(Completed)
    │
    └─► loop until Complete/Failed/Waiting
```

- `loopflow-engine` provides pure functions: `expand_flow()`, `next_action()`, `gather_context()`, `format_prompt()`, `build_agent_command()`
- `lfd` owns state: `WaveExecutor` persists Agent/WaveRun, manages scheduler slots, streams output via `OutputHub`

## Risks and bottlenecks

| Risk | Severity | Mitigation |
|------|----------|------------|
| Watch/cron bypass scheduler slots | Medium | Document; could add slot acquisition later |
| Interactive resumes skip slot checks | Low | Single user typically; add later if needed |
| Fork cleanup is best-effort | Low | Worktree removal can fail; orphaned dirs cleaned on next prune |
| No fork retry logic | Low | Design has `fork_attempts` placeholder; implement when needed |
| Choose always picks first option | Low | Placeholder; wire LLM choice agent when needed |

## What's not included

- **LLM-driven fork selection**: `ForkSelect::Prompt` deterministically picks first branch
- **Fork retry tracking**: No `fork_attempts` counter or max-retry behavior
- **Wave config overrides on WaveRun**: `flow`, `direction`, `area` overrides still update Wave directly
- **Container/K8s executors**: Out of scope per design doc (phase 2)

## Files changed

| Area | Files | Summary |
|------|-------|---------|
| Proto | `control.proto` | Added `WaveRun`, `ListWaveRuns`, `GetWaveRun`; removed execution fields from `Wave` |
| lfd executor | `executor.rs` | New `WaveExecutor`, `StepRunner` trait, `AgentRunner`, fork/choose execution |
| lfd loops | `common.rs`, `loop_ticker.rs`, `watch.rs`, `cron.rs`, `recovery.rs` | Shared helpers; wired to WaveExecutor |
| lfd server | `server.rs` | Added ListWaveRuns/GetWaveRun RPCs; StreamOutput via OutputHub |
| lfd store | `mod.rs`, `sqlite.rs`, `postgres.rs` | Added wave_runs table and ForkRun CRUD |
| lfd output | `output.rs` | New `OutputHub` broadcast channel |
| loopflow-engine | `flow.rs` | Added `expand_flow()`, `next_action()`, `ConcreteStep`, `ConcreteFork`, `ForkSelect` |
| loopflow-engine | `lib.rs` | Removed `runtime` and `store` modules |
| Deleted | `runtime.rs`, `store.rs`, `bin/lf-engine.rs`, `runtime_tests.rs` | Moved logic to lfd or deleted |

## Test coverage

- `cargo test -p loopflow-engine`: 166 tests pass (flow expansion, next_action, context gathering)
- `cargo test -p lfd`: 3 tests pass (sqlite/postgres store suites, PTY test ignored)
- `cargo clippy`: No warnings

## Done criteria status

| Criterion | Status |
|-----------|--------|
| `RunWave` executes flow end-to-end | ✓ |
| Interactive steps pause with WaveRunStatus::Waiting | ✓ |
| `StreamOutput` streams agent output | ✓ |
| loop/watch/cron use `executor.execute()` | ✓ |
| Fork execution with parallel branches | ✓ |
| lf-engine binary deleted | ✓ |
| Proto has WaveRun, Wave simplified | ✓ |
| Store migrated to wave_runs table | ✓ |
| Tests pass | ✓ |
