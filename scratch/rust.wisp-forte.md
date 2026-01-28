# Rust Core Engine (Stage 2)

## Problem

Loopflow needs a Rust engine that owns flow execution, prompt assembly, and run state transitions. The Python daemon will shell out to this engine for execution, but the engine provides the single source of truth for how flows work.

The engine must support tick-based execution for interactive steps—advancing one step at a time, pausing when it hits an interactive step, resuming when the user connects.

**Who benefits:** Daemon developers get a stable, fast execution layer. Remote clients (Concerto, mobile) get protocol-based access to engine behavior. Long-term, the Rust engine enables Linux containers and managed clusters.

**Why now:** The protocol schema is defined. Python flow execution is working but tightly coupled to the daemon. Extracting execution into a well-bounded Rust crate creates the isolation needed for reliable 24/7 operation.

## Approach

Build `lf-core`, a Rust crate that the daemon calls via in-process FFI (initially) or gRPC (later). The crate exposes a small API:

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

The daemon (Python or Rust) remains responsible for:
- Scheduling (when to run)
- Concurrency management (slots, PR limits)
- Watch/cron stimulus evaluation
- Database persistence (SQLite/Postgres)
- Event broadcasting to clients

The engine is responsible for:
- Flow parsing and DAG construction
- Step execution (shelling to `lf` or `claude` directly)
- Prompt assembly and token counting
- Run state machine (step_index, status)
- Worktree operations during execution

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| **Full daemon rewrite** | Rust handles everything | Too much at once; high risk of UX regression |
| **Shell to Python lf** | Daemon shells to `lf --step` | Already works; but no isolation, can't run on Linux without Python |
| **gRPC service** | Separate process for engine | Good for isolation but adds IPC overhead; defer to Stage 3 |
| **WASM module** | Portable, sandboxed | Sandboxing constraints would limit git/worktree ops |

The chosen approach (Rust crate with FFI) keeps execution fast and in-process while establishing the interface that gRPC will use later.

## Key decisions

### 1. Crate boundary: `lf-core`

Single crate containing all engine logic. No workspace splitting until we have multiple crates with clear dependencies.

```
rust/
  lf-core/
    Cargo.toml
    src/
      lib.rs          # Public API
      flow/           # Flow parsing, DAG, step types
      runtime/        # Tick executor, state machine
      prompt/         # Context gathering, token counting
      worktree/       # Git worktree operations
      git/            # Status, diff, commit
      event/          # Structured events
```

### 2. Database interaction via trait

The engine doesn't own the database. It receives a trait object:

```rust
pub trait RunStore {
    fn get_run(&self, id: &str) -> Result<FlowRun, StoreError>;
    fn update_run(&self, run: &FlowRun) -> Result<(), StoreError>;
    fn create_step_run(&self, step_run: &StepRun) -> Result<(), StoreError>;
}
```

This lets the Python daemon pass a wrapper around its SQLite connection (via PyO3) and the future Rust daemon pass its own implementation.

### 3. Step execution: shell to `lf` initially

The engine shells to `lf --step <name> --worktree <path> --direction <d1>,<d2>` rather than re-implementing prompt formatting and model invocation. This preserves Python's prompt assembly during the transition.

Later (Stage 4), the engine will call `lf-core` prompt assembly and invoke models directly.

```rust
fn run_step(step: &Step, worktree: &Path, directions: &[String]) -> StepResult {
    let mut cmd = Command::new("lf");
    cmd.arg("--step").arg(&step.name);
    cmd.arg("--worktree").arg(worktree);
    cmd.arg("--auto");
    if !directions.is_empty() {
        cmd.arg("--direction").arg(directions.join(","));
    }
    let output = cmd.output()?;
    // Parse exit code, capture stdout/stderr
}
```

### 4. Token counting: tiktoken-rs or byte fallback

Use `tiktoken-rs` for cl100k_base tokenizer if available and accurate. If the Rust binding drifts from Python tiktoken, fall back to byte-based limits with a 4x safety factor (1 token ≈ 4 bytes on average).

Document the fallback clearly:

```rust
pub fn count_tokens(text: &str) -> usize {
    match tiktoken_count(text) {
        Some(count) => count,
        None => {
            // Fallback: conservative byte estimate
            (text.len() / 3).max(1)
        }
    }
}
```

### 5. Flow parsing: exact parity with Python

Parse flow YAML to the same structure as Python. Fork, Choose, LoopUntilEmpty must behave identically.

```rust
pub enum FlowItem {
    Step(Step),
    Fork { branches: Vec<FlowItem>, synthesize: Option<String> },
    Choose { prompt: String, options: HashMap<String, Vec<FlowItem>> },
    LoopUntilEmpty { steps: Vec<FlowItem> },
}

pub struct Flow {
    pub name: String,
    pub items: Vec<FlowItem>,
}
```

Golden tests: parse Python flow YAML → serialize to JSON, parse same YAML in Rust → serialize to JSON, compare.

### 6. Events: proto-based

Engine emits events matching `engine/v1/engine.proto` types. The daemon receives these and broadcasts via its Subscribe stream.

```rust
pub enum EngineEvent {
    StepStarted { run_id: String, step: String, timestamp: SystemTime },
    StepCompleted { run_id: String, step: String, exit_code: i32, timestamp: SystemTime },
    StepFailed { run_id: String, step: String, error: String, timestamp: SystemTime },
    FlowCompleted { run_id: String, timestamp: SystemTime },
    FlowFailed { run_id: String, error: String, timestamp: SystemTime },
}
```

### 7. Error handling: typed errors

```rust
#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    #[error("flow not found: {0}")]
    FlowNotFound(String),
    #[error("step not found: {0}")]
    StepNotFound(String),
    #[error("invalid flow: {0}")]
    InvalidFlow(String),
    #[error("execution failed: {0}")]
    ExecutionFailed(String),
    #[error("worktree error: {0}")]
    WorktreeError(String),
    #[error("store error: {0}")]
    StoreError(#[from] StoreError),
    #[error("io error: {0}")]
    IoError(#[from] std::io::Error),
}
```

## Scope

**In scope:**
- Flow YAML parsing to FlowItem DAG
- tick_flow() state machine with TickResult
- Step execution via subprocess to `lf`
- Worktree create/find/remove
- Git status/diff (read-only initially)
- Token counting with tiktoken-rs or fallback
- Structured events for step/flow lifecycle
- PyO3 bindings for Python daemon integration

**Out of scope:**
- Full prompt rendering (stays in Python lf for now)
- Direct model invocation
- Watch/cron stimulus evaluation (stays in daemon)
- gRPC server (Stage 3)
- Postgres backend (Stage 5)

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
