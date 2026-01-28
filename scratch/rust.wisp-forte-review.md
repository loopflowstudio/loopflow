# Rust Core Engine Stage 2 Review

## What was implemented
- Added a Rust workspace with a new `lf-core` crate that defines the core engine API.
- Implemented flow parsing (steps, fork/choose/loop structures) and loaders for flows, steps, and directions.
- Implemented tick-based runtime execution for linear step flows, plus a shell-based step runner.
- Added prompt context structures, token counting fallback, and basic trimming behavior.
- Added git/worktree helpers and core error types, plus unit tests for parsing, runtime ticking, and token counting.

## Key choices
- **Trait-based RunStore** to keep persistence out of the core crate and allow Python or Rust daemons to provide storage.
- **Shell to `lf`** for step execution to preserve existing Python prompt assembly and model invocation.
- **YAML parsing via `serde_yaml::Value`** for parity with Python flows and to tolerate heterogeneous flow structures.
- **Tick runtime limited to `Step` items** to keep state machine simple while non-linear flow execution is deferred.

## How it fits together
`load_flow` parses `.lf/flows` into a `Flow` DAG, `tick_flow` advances a stored `FlowRun` by one step, and `CommandStepRunner` shells to `lf` for execution. Prompt context structures live alongside this runtime, with minimal token counting and trimming to support later integration.

## Risks and bottlenecks
- Tick execution currently fails on non-step flow items; flows using fork/choose/loop will stop with a failure state.
- Shelling to `lf --step` assumes CLI flag compatibility; any drift will break execution.
- Token counting is a byte-based heuristic; large prompt components may under/over count until tiktoken integration is added.

## What's not included
- Full prompt rendering and model invocation in Rust.
- Execution of fork/choose/loop flow items.
- A concrete RunStore implementation or database layer.
- Structured engine events beyond data types.
