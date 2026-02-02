# Simplification Opportunities

## Product intent

Loopflow wants to be a single tool for running AI-assisted coding workflows: steps (single prompts), flows (sequences of steps), and waves (automated triggers). Users should be able to type `lf debug` and have the right context assembled, the right agent launched, and the result tracked—whether running interactively at a terminal or automatically in the background.

## Opportunity 1: Collapse the daemon's role into orchestration-only

**Misalignment**: The product has two execution models fighting each other. The CLI (`lf`) can run flows directly using `tick_flow()` from the engine. The daemon (`lfd`) can also run flows using the same engine. But the daemon adds a layer of stimulus management, slot scheduling, and persistence that the CLI ignores entirely.

The roadmap says "daemon is for waves and automation; CLI is for humans." But the implementation makes them parallel execution paths rather than layered ones. The CLI builds its own `InMemoryStore` and runs flows in a loop. The daemon builds a `SharedStore` and runs flows in a timer-driven loop. Same engine, two different orchestration layers.

**Symptom**:
- `lf/src/commands/flow.rs` creates an `InMemoryStore` and calls `tick_flow_with_runner()` in a loop
- `lfd/src/loops/loop_ticker.rs` creates an adapter and calls `tick_flow()` on a timer
- Both implement "run a flow to completion" but with different persistence, error handling, and lifecycle semantics
- No code path for "CLI asks daemon to run a flow" despite the gRPC API existing

**Realignment**: Make the daemon the only place that runs multi-step flows with persistence. The CLI becomes a thin client that either:
1. Runs single steps directly (stateless, current behavior)
2. Submits flow runs to the daemon and streams output back

The engine provides the state machine (`tick_flow`). The daemon provides orchestration (scheduling, persistence, stimuli). The CLI provides the interface. Each layer has one job.

**Cascade**:
- Delete `InMemoryStore` from the CLI; it's a test double pretending to be production code
- Flow resumption after interactive steps becomes obvious: daemon tracks state, CLI reconnects
- Wave management commands (`lf wave create`) become natural: they're just RPC calls
- The "stimulus" concept can live entirely in the daemon without touching the engine

---

## Opportunity 2: Unify the store abstraction

**Misalignment**: The engine defines `RunStore` (get_run, update_run, create_agent). The daemon defines a richer store with Wave, Stimulus, Agent, ForkRun, PendingActivation, Worktree tables. An adapter bridges them, but it's awkward.

The engine's `RunStore` speaks in terms of "runs" (transient execution state). The daemon's store speaks in terms of "waves" (persistent automation definitions). These are different concepts getting merged through an adapter that loses information both ways.

**Symptom**:
- `lfd/src/store/lf_core_adapter.rs` exists solely to translate between two store abstractions
- The engine's `WaveRun` struct has `id, flow, directions, areas, repo, status, step_index, worktree`
- The daemon's `Wave` proto has `id, name, repo, flow, status, paused, consecutive_failures, error`
- These overlap but don't match; the adapter copies fields back and forth
- Stimulus evaluation logic (`watch_poller.rs`, `cron_poller.rs`) queries the daemon store directly, bypassing the engine entirely

**Realignment**: The engine should not have a store abstraction. It should be pure: take flow state in, return new state out. The daemon owns all persistence.

```rust
// Engine becomes stateless
fn tick_flow(run: &WaveRun, step_runner: &impl StepRunner) -> TickResult {
    // No store calls; just state transitions
}

// Daemon handles persistence
async fn run_wave(&self, wave_id: &str) {
    let wave = self.store.get_wave(wave_id)?;
    let mut run = wave.to_run_state();

    loop {
        match tick_flow(&run, &self.runner) {
            TickResult::StepComplete(new_state) => {
                run = new_state;
                self.store.update_wave_state(&wave_id, &run)?;
            }
            // ...
        }
    }
}
```

**Cascade**:
- Delete `RunStore` trait from the engine
- Delete `LfCoreStoreAdapter` from the daemon
- Engine becomes testable without any storage mocks
- The daemon's store schema becomes the single source of truth
- Direction merging, config loading, worktree management all live in the daemon's orchestration layer

---

## Opportunity 3: Make steps the only primitive

**Misalignment**: The engine has three concepts that are all "run a prompt": Step, FlowItem::Step, and inline prompts (`lf : "prompt"`). Flows add Fork, Choose, and LoopUntilEmpty. But the product intent is simpler: users want to run prompts with context.

Fork is "run these steps in parallel." Choose is "ask an agent which step to run." LoopUntilEmpty is "keep running a step until a condition." These are orchestration patterns, not new primitives. The engine treats them as first-class flow items, which adds parsing complexity and special-case handling.

**Symptom**:
- `flow.rs` has 200+ lines parsing Fork with branches, synthesize steps, Choose with options and prompts
- `runtime.rs` has separate code paths for `run_step_item`, `run_fork_item`, `run_choose_item`, `run_loop_item`
- Fork creates `ForkRun` records with `wave_id`, `branch_name`, `status`, `merge_point`
- The CLI's flow runner doesn't support Fork properly (uses `InMemoryStore` with incomplete ForkRun tracking)

**Realignment**: Fork, Choose, and LoopUntilEmpty become orchestration strategies in the daemon, not flow parsing primitives. A flow is a list of steps with optional control metadata.

```yaml
# Instead of special syntax for Fork:
items:
  - fork:
      branches:
        - steps: [a, b]
        - steps: [c, d]
      synthesize: merge

# Use declarative metadata:
items:
  - step: a
    parallel_group: branch-1
  - step: b
    parallel_group: branch-1
    after: a
  - step: c
    parallel_group: branch-2
  - step: d
    parallel_group: branch-2
    after: c
  - step: merge
    after: [b, d]  # runs after both branches complete
```

This is more verbose but makes the data model flat: flows are DAGs of steps, not recursive trees of items. The daemon interprets the DAG; the engine just runs individual steps.

**Cascade**:
- Delete `FlowItem` enum; flows become `Vec<Step>` with dependency metadata
- Delete Fork/Choose/Loop handling from the engine
- The daemon's scheduler already has "run steps in parallel" via slot management; extend it to handle step dependencies
- Flow files become simpler to parse and validate
- Worktree creation becomes explicit: a step can declare `worktree: new` and the daemon creates one before running

---

## Aligned areas

**Prompt assembly (`prompt.rs`)**: The context gathering logic matches the product well. Users specify directions and areas; the engine finds the right files, diffs, and clips them together. The component model (PromptComponents) is clean.

**Agent invocation (`agent.rs`)**: Building CLI commands for claude/codex/gemini is straightforward. The abstraction is right: take a prompt and config, return a subprocess. No unnecessary generalization.

**Config merging (`config.rs`)**: Global + repo + CLI override is the right hierarchy. The implementation is simple. Keep it.

**Step/direction discovery (`lf/discovery.rs`)**: Finding markdown files in `.lf/steps`, `.claude/commands`, global directories—this is well-aligned with how users organize their prompts.

**gRPC API design (`control.proto`)**: The service methods (CreateWave, RunWave, StopWave, ListStimuli) match what users need. The proto is clean. The problem is that nothing calls it.
