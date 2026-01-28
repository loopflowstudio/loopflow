# Rust Core Engine (Stage 2)

## Goal
Build the Rust engine that owns flow execution, prompt assembly, and run state transitions. The Python daemon shells out to this engine for execution, but the engine is the single source of truth for how flows work.

## Scope
- Flow parsing and validation
- Run state machine (steps, retries, failures)
- Tick-based execution for interactive steps
- Prompt assembly pipeline and token counting policy
- Worktree and git operations
- Event model and internal logging

## Non-goals
- Full daemon scheduling and triggers
- Cluster deployment
- Full prompt rendering/model invocation in Rust (Stage 4)
- Engine contract gRPC implementation (Stage 3)

## Approach
Build `lf-core`, a Rust crate called by the daemon via in-process FFI (initially) and later via gRPC. Expose a small API:

```rust
// Core execution
fn tick_flow(run_id: &str, db: &Database) -> TickResult;
fn gather_context(opts: &GatherContextOpts) -> PromptComponents;
fn run_step(step: &Step, worktree: &Path, direction: &[String]) -> StepResult;

// Artifact loading
fn load_flow(name: &str, repo: &Path) -> Result<Flow, LoadError>;
fn load_step(name: &str, repo: &Path) -> Result<Step, LoadError>;
fn load_direction(name: &str, repo: &Path) -> Result<Direction, LoadError>;
```

The daemon remains responsible for scheduling, concurrency limits, stimulus evaluation, persistence, and broadcasting. The engine owns flow parsing, step execution, prompt assembly, and run state transitions.

## Core modules
- `flow`: parse/load steps and DAGs
- `runtime`: run state machine, tick-based executor
- `prompt`: context gathering, token counting, trimming
- `worktree`: create/find/remove worktrees
- `git`: status/diff utilities
- `event`: structured events for runs and steps

## Tick-based execution
Interactive steps require tick-based execution. The engine advances step-by-step, pausing when it hits an interactive step and resuming when the user connects.

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

## Protocol alignment
Loopflow standardizes daemon/client integration on a two-tier protobuf schema with gRPC as the primary transport and JSON-over-HTTP as a compatibility layer.

**Decisions:**
- Control plane (lf/Concerto → lfd) is public; engine contract (lfd → lf-core) is internal.
- Protobuf-first: gRPC is primary; JSON is derived from proto.
- No WebSocket streaming; use server-side streaming via `Subscribe`.
- Idempotency keys on mutations for safe retries.
- Typed errors include machine code, human message, retryability, and delay.

**Remaining gaps:**
- Engine contract gRPC implementation (streaming execution not wired)
- Some control-plane RPCs still partial across gRPC/HTTP parity
- Swift client integration still pending

## Key decisions
1. **Crate boundary:** single crate `lf-core` containing all engine logic.
2. **Database via trait:** RunStore trait keeps persistence out of core.
3. **Step execution:** shell to `lf --step <name> --worktree <path> --direction <d1>,<d2>` initially.
4. **Token counting:** tiktoken-rs for cl100k_base if accurate; fallback to byte-based estimates.
5. **Flow parsing:** exact parity with Python structure (Fork/Choose/LoopUntilEmpty).
6. **Events:** engine emits proto-aligned lifecycle events.
7. **Typed errors:** structured error surface for callers.

## Current implementation status
- Rust workspace with `lf-core` crate and public API.
- Flow parsing (step/fork/choose/loop) and loaders for flows/steps/directions.
- Tick-based runtime execution for linear steps; shells to `lf` for execution.
- Prompt context structs, token-counting fallback, and trimming behavior.
- Git/worktree helpers, core error types, and unit tests.

## Risks and bottlenecks
- Tick execution fails on non-step flow items (fork/choose/loop not executed yet).
- Shelling to `lf --step` depends on CLI flag compatibility.
- Token counting is a heuristic until tiktoken integration is added.
- Worktree edge cases on macOS vs Linux.

## Done when
```bash
# Parse a flow, verify structure matches Python
cargo test --package lf-core flow_parsing_parity

# Tick through a simple auto flow
cargo test --package lf-core tick_auto_flow_end_to_end

# Tick to interactive step, verify WAITING state
cargo test --package lf-core tick_interactive_pauses

# Python daemon calls Rust engine
pytest tests/test_lfd.py -k rust_engine

# Count tokens with tiktoken-rs
cargo test --package lf-core token_counting

# Golden flow set (10 representative flows)
cargo test --package lf-core golden_flows
```

The engine can execute the `ship` flow (implement → compress → gate → consolidate) end-to-end with the same observable behavior as Python.

## Open questions
- Which Python behaviors should be left behind vs matched exactly?
- How much of prompt rendering should be configurable vs hard-coded?
- Which tokenizer is acceptable, and when do we fall back to byte limits?
- Does the `lf` CLI require different flags than `--step/--worktree/--direction`?
