# loopflow-engine

Rust engine for loopflow. Owns flow execution, context assembly, and agent invocation.

## Quick Start

```bash
cargo build                   # Build
cargo test                    # Run tests
cargo clippy -- -D warnings   # Lint
cargo fmt                     # Format
```

## Architecture

```
loopflow-engine/
├── agent.rs     # Agent invocation (Claude, Codex, Gemini)
├── config.rs    # Config loading (~/.lf/, .lf/)
├── flow.rs      # Flow/step/direction parsing from YAML
├── prompt.rs    # Context gathering and prompt formatting
├── runtime.rs   # Tick-based execution engine
├── python.rs    # PyO3 bindings for Python integration
├── store.rs     # RunStore trait for persistence
├── git.rs       # Git operations (rebase, push, branch)
├── worktree.rs  # Worktree creation/removal
├── error.rs     # Typed errors with thiserror
└── event.rs     # Lifecycle events for runs
```

## Core API

```rust
use loopflow_engine::{load_flow, tick_flow, RunId, RunStore};

// Load a flow from .lf/flows/
let flow = load_flow("ship", &repo_path)?;

// Tick through execution (advances one step)
let run_id = RunId::new("run-123");
let result = tick_flow(&run_id, &store)?;
match result {
    TickResult::StepComplete => println!("Step done, continue ticking"),
    TickResult::FlowComplete => println!("All steps done"),
    TickResult::WaitingInteractive => println!("Paused for user input"),
    TickResult::StepFailed => println!("Step failed"),
}
```

## Key Types

### RunId (newtype)

```rust
let id = RunId::new("my-run");
println!("{}", id);           // Display
println!("{}", id.as_str());  // Borrow as &str
```

Wraps `String` to prevent mixing run IDs with other strings. Implements `Hash` for use as map keys.

### FlowItem (enum)

```rust
pub enum FlowItem {
    Step(Step),                              // Single step
    Fork { branches, synthesize },           // Parallel execution
    Choose { prompt, options },              // User choice
    LoopUntilEmpty { steps },                // Repeat until done
}
```

Rust enums hold data. Pattern matching forces you to handle all variants.

### Result and Option

```rust
// Option<T> for "not found"
fn find_config() -> Option<Config> { ... }

// Result<T, E> for failures
fn load_flow(name: &str) -> Result<Flow, LoadError> { ... }

// ? propagates errors
let flow = load_flow("ship", &repo)?;  // Returns early if Err
```

### RunStore (trait)

```rust
pub trait RunStore {
    fn get_run(&self, id: &RunId) -> Result<FlowRun, StoreError>;
    fn update_run(&self, run: &FlowRun) -> Result<(), StoreError>;
    fn create_step_run(&self, step_run: &StepRun) -> Result<(), StoreError>;
}
```

Implement this to plug in your storage backend (SQLite, in-memory, etc.).

## Flow Execution

The engine uses **tick-based execution**. Each `tick_flow` call advances one step:

```
tick_flow() → StepComplete    (auto step finished, call again)
           → FlowComplete     (all steps done)
           → WaitingInteractive (paused at interactive step)
           → StepFailed       (step errored)
```

Interactive steps pause execution until the user connects. The daemon calls `tick_flow` again when `StepRunEnd` signals completion.

## Style Guide

### Error Handling

- `thiserror` for library errors callers can match on
- Return `Option<T>` for "not found"
- Return `Result<T, E>` for failures
- Use `expect("reason")` over `unwrap()` outside tests

### Naming

- Conversion: `as_` (cheap), `to_` (allocates), `into_` (consumes)
- No `get_` prefix: `fn name(&self)` not `fn get_name(&self)`
- Newtypes for domain concepts: `RunId(String)` not `type RunId = String`

### Derives

Always derive `Debug` on public types. Add `Clone`, `PartialEq`, `Default` where sensible:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FlowRunStatus { ... }
```

### Traits for Injection

Use traits for dependencies to enable testing:

```rust
pub trait StepRunner {
    fn run(&self, step: &Step, ...) -> Result<StepResult, CoreError>;
}

// Real implementation
pub struct CommandStepRunner;

// Test implementation
struct FakeRunner { exit_code: i32 }
```

## Testing

```rust
// Tests can use unwrap() freely
#[test]
fn tick_completes_flow() {
    let store = MemoryStore::new(run);
    let result = tick_flow(&run_id, &store).unwrap();
    assert_eq!(result, TickResult::FlowComplete);
}
```

Mock via traits: implement `RunStore` and `StepRunner` with test doubles.

## Token Counting

Uses tiktoken-rs with cl100k_base encoding for accurate token counts:

```rust
let tokens = count_tokens("hello world");  // ~3 tokens
```

Falls back to bytes/3 heuristic if tiktoken fails to load.

## Status

Working:
- Flow parsing (step/fork/choose/loop)
- Tick-based execution for all flow items
- Fork execution with parallel worktrees
- Choose execution (deterministic selection)
- LoopUntilEmpty execution with wave termination
- Agent invocation (Claude, Codex, Gemini)
- Context assembly with docs, diff, clipboard
- tiktoken-rs token counting
- Config loading with global/repo merging
- PyO3 bindings for Python integration

Not yet implemented:
- Summary loading
- Bundled LOOPFLOW.md embedding
- LLM-based choose prompt evaluation
