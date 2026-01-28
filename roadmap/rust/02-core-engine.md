# Rust Roadmap: Core Engine (Stage 2)

Build the Rust engine that implements flow execution and core behavior.

## Goal
Implement the single source of truth for flow evaluation, prompt assembly, and run state transitions in Rust.

## Scope
- Flow parsing and validation
- Run state machine (steps, retries, failures)
- Tick-based execution for interactive steps
- Prompt assembly pipeline
- Worktree and git operations
- Event model and internal logging
- Token counting policy and limits

## Non-goals
- Full daemon scheduling and triggers
- Cluster deployment

## Core modules
- `flow`: parse/load steps and DAGs
- `runtime`: run state machine, tick-based executor
- `prompt`: context assembly, file selection, formatting
- `worktree`: create/find/remove worktrees
- `git`: status, diff, commit utilities
- `event`: structured events for runs and steps

## Tick-based execution
Flows containing interactive steps require tick-based execution. The engine advances step-by-step, pausing when it hits an interactive step.

**FlowRun state:**
- `step_index`: current position in flow (persisted)
- `status`: running | waiting | completed | failed

**TickResult enum:**
- `StepComplete` — auto step finished, continue ticking
- `FlowComplete` — all steps done
- `WaitingInteractive` — paused at interactive step, awaiting user connect
- `StepFailed` — step errored, flow stops

**tick_flow() behavior:**
1. Load FlowRun from DB
2. Get next step at `step_index`
3. If interactive: create WAITING StepRun, emit `wave.waiting`, return `WaitingInteractive`
4. If auto: shell to `lf --step <step> --worktree <path>`, advance `step_index`
5. Return `StepComplete` or `StepFailed`

The daemon calls `tick_flow` initially and again when `StepRunEnd` signals interactive step completion.

## Interfaces
- Rust crate API for daemon use
- Protocol adapter layer for external clients

## Risks
- Parity gaps vs Python flow behavior
- Worktree edge cases on macOS vs Linux

## Success criteria
- Engine can execute a flow end-to-end with the same output as Python.
- Run state machine emits correct events for every step.
- Behavior differences are documented and intentional.

## Open questions
- Which Python behaviors should be left behind vs matched exactly?
- How much of prompt rendering should be configurable vs hard-coded?
- Which tokenizer is acceptable, and when do we fall back to byte limits?
