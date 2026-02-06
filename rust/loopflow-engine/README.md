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
├── agent.rs     # Agent command building (Claude, Codex, Gemini)
├── config.rs    # Config loading (~/.lf/, .lf/)
├── flow.rs      # Flow/step/direction parsing from YAML
├── prompt.rs    # Context gathering and prompt formatting
├── git.rs       # Git operations (rebase, push, branch)
├── worktree.rs  # Worktree creation/removal
├── error.rs     # Typed errors with thiserror
└── event.rs     # Lifecycle events for runs
```

## Core API

```rust
use loopflow_engine::{expand_flow, load_flow, next_action, FlowAction};

// Load a flow from .lf/flows/
let flow = load_flow("ship", &repo_path)?;
let plan = expand_flow(&flow, &repo_path)?;

// Determine what to do next
match next_action(&plan, 0) {
    FlowAction::RunStep { step } => println!("Run step: {}", step.step.name),
    FlowAction::WaitInteractive { step } => println!("Wait at: {}", step.step.name),
    FlowAction::Fork { .. } => println!("Fork branches"),
    FlowAction::Complete => println!("All steps done"),
}
```

## Key Types

### FlowItem (enum)

```rust
pub enum FlowItem {
    Step(Step),                                        // Single step
    Fork { branches, select, synthesize },             // Fork or choose
    FlowRef(String),                                   // Nested flow by name
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

## Flow Execution

The engine is **stateless**. It exposes pure helpers for flow parsing, prompt building,
and agent command construction. Execution lives in lfd.

## Style Guide

### Error Handling

- `thiserror` for library errors callers can match on
- Return `Option<T>` for "not found"
- Return `Result<T, E>` for failures
- Use `expect("reason")` over `unwrap()` outside tests

### Naming

- Conversion: `as_` (cheap), `to_` (allocates), `into_` (consumes)
- No `get_` prefix: `fn name(&self)` not `fn get_name(&self)`
- Newtypes for domain concepts when needed (avoid `type Alias = String`)

### Derives

Always derive `Debug` on public types. Add `Clone`, `PartialEq`, `Default` where sensible:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Step { ... }
```

## Token Counting

Uses tiktoken-rs with cl100k_base encoding for accurate token counts:

```rust
let tokens = count_tokens("hello world");  // ~3 tokens
```

Falls back to bytes/3 heuristic if tiktoken fails to load.

## Status

Working:
- Flow parsing (step/fork/flow refs)
- Flow expansion via `expand_flow`
- Flow action selection via `next_action` on expanded plans
- Agent invocation (Claude, Codex, Gemini)
- Context assembly with docs, diff, clipboard
- tiktoken-rs token counting
- Config loading with global/repo merging
- PyO3 bindings available via rust/loopflow-py

Not yet implemented:
- Summary loading
- Bundled LOOPFLOW.md embedding
- LLM-based fork prompt selection
