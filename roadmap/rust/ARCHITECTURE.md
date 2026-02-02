# Rust Architecture

Technical architecture of the Rust components as they exist today.

## Overview

Two crates in `rust/`:

| Crate | Purpose | Status |
|-------|---------|--------|
| `loopflow-engine` | Flow execution library | Feature-complete |
| `lfd` | Daemon (gRPC + HTTP server) | Infrastructure complete, not primary path |

The gap: Python `lf` CLI bypasses lfd entirely. The Rust daemon exists but isn't used.

---

## loopflow-engine

Library for flow execution. Also builds `lf-engine` binary for git operations.

### Modules

```
rust/loopflow-engine/src/
├── lib.rs          # Public exports
├── agent.rs        # Agent invocation (claude/codex/gemini)
├── config.rs       # Config loading and merging
├── flow.rs         # Flow parsing (YAML → FlowItem)
├── git.rs          # Git operations (rebase, push, land, PR)
├── prompt.rs       # Context gathering and prompt assembly
├── runtime.rs      # Tick-based flow execution
├── store.rs        # RunStore trait (persistence abstraction)
├── worktree.rs     # Git worktree management
├── python.rs       # PyO3 bindings (optional feature)
└── bin/
    └── lf-engine.rs  # CLI for git operations
```

### Flow Parsing (`flow.rs`)

Parses YAML/JSON flow definitions from `.lf/flows/`.

```rust
pub enum FlowItem {
    Step {
        name: String,
        interactive: bool,
        directions: Vec<String>,
    },
    Fork {
        branches: Vec<ForkBranch>,
        synthesize: Option<String>,
    },
    Choose {
        options: Vec<ChooseOption>,
    },
    LoopUntilEmpty {
        source: String,
        step: Box<FlowItem>,
    },
}

pub struct Flow {
    pub name: String,
    pub items: Vec<FlowItem>,
}
```

Loads from `.lf/flows/*.yaml`, `.lf/flows/*.yml`, `.lf/flows/*.json`.

### Context Gathering (`prompt.rs`)

Assembles context for prompts:

```rust
pub struct ContextComponents {
    pub docs: Vec<Document>,
    pub diff: Option<String>,
    pub diff_files: Vec<String>,
    pub directions: Vec<String>,
    pub summaries: Vec<String>,
    pub clipboard: Option<String>,
}

pub fn gather_context(config: &ContextConfig) -> Result<ContextComponents>;
pub fn count_tokens(text: &str) -> usize;  // tiktoken cl100k_base
pub fn analyze_tokens(components: &ContextComponents) -> TokenAnalysis;
pub fn trim_context(components: ContextComponents, budget: usize) -> ContextComponents;
pub fn format_prompt(components: &ContextComponents, step: &Step) -> String;
```

Token counting uses tiktoken with byte-estimation fallback. Trimming drops summaries first, then docs, then diff to fit budget.

### Agent Invocation (`agent.rs`)

Spawns coding agents:

```rust
pub enum AgentBackend { Claude, Codex, Gemini }

pub struct AgentConfig {
    pub backend: AgentBackend,
    pub model: String,
    pub prompt: String,
    pub working_dir: PathBuf,
    pub auto_mode: bool,
    pub streaming: bool,
    pub chrome: bool,
    pub skip_permissions: bool,
}

pub fn launch_agent(config: AgentConfig) -> Result<AgentOutput>;
pub fn check_cli_available(backend: AgentBackend) -> bool;
```

Command construction per backend:
- **Claude**: `claude --model <model> --print` with `--dangerously-skip-permissions` for auto mode
- **Codex**: `codex exec` with sandbox levels and approval policies
- **Gemini**: Model selection with output format control

### Git Operations (`git.rs`)

Comprehensive git/GitHub operations:

```rust
// Branch operations
pub fn create_branch(repo: &Path, name: &str) -> Result<()>;
pub fn delete_local_branch(repo: &Path, name: &str) -> Result<()>;
pub fn delete_remote_branch(repo: &Path, name: &str) -> Result<()>;
pub fn checkout(repo: &Path, branch: &str) -> Result<()>;
pub fn is_ancestor(repo: &Path, ancestor: &str, descendant: &str) -> Result<bool>;

// Sync operations
pub fn rebase(repo: &Path, onto: &str) -> Result<RebaseResult>;
pub fn push(repo: &Path) -> Result<()>;
pub fn push_with_upstream(repo: &Path, branch: &str) -> Result<()>;
pub fn sync_main(repo: &Path) -> Result<()>;

// Landing
pub enum LandStrategy { SquashMerge, LocalMerge }
pub fn land(repo: &Path, strategy: LandStrategy) -> Result<()>;

// PR operations
pub fn pr_exists(repo: &Path) -> Result<bool>;
pub fn pr_create_draft(repo: &Path, title: &str, body: &str) -> Result<String>;
pub fn pr_merge_squash_auto(repo: &Path) -> Result<()>;
```

### Tick Execution (`runtime.rs`)

Core flow execution engine:

```rust
pub struct WaveRun {
    pub id: String,
    pub flow: Flow,
    pub directions: Vec<String>,
    pub areas: Vec<PathBuf>,
    pub repo: PathBuf,
    pub status: WaveRunStatus,
    pub step_index: usize,
    pub worktree: Option<PathBuf>,
    pub current_step: Option<String>,
    pub error: Option<String>,
}

pub enum WaveRunStatus { Pending, Running, Waiting, Completed, Failed }

pub fn tick_flow<S: RunStore>(run: &mut WaveRun, store: &S) -> Result<TickResult>;

pub enum TickResult {
    Continue,           // More work to do
    WaitingForAgent,    // Agent running
    WaitingForConnect,  // Interactive step, waiting for user
    Completed,          // Flow finished
    Failed(String),     // Error occurred
}
```

### Config Loading (`config.rs`)

Merges global and repo config:

```rust
pub struct Config {
    pub agent_model: String,
    pub yolo: bool,
    pub chrome: bool,
    pub push: bool,
    pub pr: bool,
    pub land: bool,
    pub context: Vec<String>,
    pub exclude: Vec<String>,
    pub summaries: Vec<String>,
    pub ide: Option<String>,
    pub interactive: bool,
    pub token_budget: usize,
}

pub fn load_config(repo: &Path) -> Result<Config>;
```

Loads from `~/.lf/config.yaml` (global) and `.lf/config.yaml` (repo). Repo overrides global; additive keys combine.

### lf-engine Binary

CLI exposing git operations as subcommands with JSON output:

```
lf-engine rebase --onto main
lf-engine push --force-with-lease
lf-engine land --strategy squash-merge
lf-engine pr-create-draft --title "feat: ..." --body "..."
lf-engine pr-exists
lf-engine branch --name feature-x
lf-engine sync-main
```

Python `lf` CLI calls these subcommands for git work.

---

## lfd

Daemon providing gRPC and HTTP APIs for wave orchestration.

### Architecture

```
rust/lfd/src/
├── main.rs         # Server startup, signal handling
├── server.rs       # gRPC service implementation
├── http.rs         # HTTP endpoints (/health, /status, /metrics)
├── scheduler.rs    # Slot management (concurrency control)
├── sessions.rs     # PTY command execution
├── id.rs           # LfdId newtype (UUID-based)
├── store/
│   ├── mod.rs      # Store trait + tests
│   ├── sqlite.rs   # SQLite implementation
│   └── postgres.rs # PostgreSQL implementation
└── loops/
    ├── mod.rs          # Loop orchestration
    ├── loop_ticker.rs  # Loop stimulus handler (5s)
    ├── watch.rs        # Watch stimulus handler (30s)
    ├── cron.rs         # Cron stimulus handler
    └── recovery.rs     # Stuck agent cleanup
```

### Server Startup

```rust
#[tokio::main]
async fn main() {
    let store = match env::var("LFD_STORAGE") {
        Ok(s) if s == "postgres" => postgres_store(),
        _ => sqlite_store(),
    };

    let scheduler = Scheduler::new(max_slots);
    let cancel = CancellationToken::new();

    // Background loops
    spawn_loop_ticker(store.clone(), scheduler.clone(), cancel.clone());
    spawn_watch_poller(store.clone(), cancel.clone());
    spawn_cron_poller(store.clone(), cancel.clone());
    spawn_recovery_loop(store.clone(), cancel.clone());

    // Servers
    tokio::spawn(grpc_server(store.clone(), scheduler.clone()));
    tokio::spawn(http_server(store.clone(), scheduler.clone()));

    signal::ctrl_c().await;
    cancel.cancel();
}
```

Environment variables:
- `LFD_STORAGE`: `sqlite` (default) or `postgres`
- `LFD_DATABASE_URL`: Postgres connection string
- `LFD_GRPC_ADDR`: gRPC listen address (default: `127.0.0.1:50051`)
- `LFD_HTTP_ADDR`: HTTP listen address (default: `127.0.0.1:8080`)
- `LFD_MAX_SLOTS`: Max concurrent agents (default: 4)

### gRPC Service

40+ RPC methods:

**Wave Management**: ListWaves, GetWave, CreateWave, UpdateWave, DeleteWave, CloneWave, RunWave, StopWave, ConnectWave

**Stimulus Management**: ListStimuli, GetStimulus, CreateStimulus, UpdateStimulus, DeleteStimulus

**Agent Tracking**: ListAgents, GetAgentHistory, StartAgent, EndAgent

**Events**: Subscribe, Notify, StreamOutput

**Scheduler**: GetSchedulerStatus, AcquireSlot, ReleaseSlot

### HTTP Interface

```
GET /health  → { status, uptime_seconds, database, waves_running, agents_active }
GET /status  → { pid, waves_defined, waves_running, agents_active, slots_used, slots_total }
GET /metrics → { waves_total, waves_running, agents_active, slots_used, slots_total }
```

### Storage

**Store Trait**: CRUD for waves, stimuli, pending activations, fork runs, agents. Health checks and schema versioning.

**SQLite**: Embedded database with rusqlite. Single-threaded, synchronous.

**PostgreSQL**: Async via tokio-postgres + deadpool connection pooling.

Tables: `waves`, `stimuli`, `pending_activations`, `fork_runs`, `agents`

### Background Loops

- **Loop Ticker** (5s): Finds waves with `STIMULUS_LOOP`, calls `tick_flow()`
- **Watch Poller** (30s): Checks main branch SHA, creates `PendingActivation` for coalescing
- **Cron Poller**: Evaluates cron expressions, triggers waves on schedule
- **Recovery Loop**: Detects stuck agents, cleans up stale processes

### PTY Sessions

For interactive step execution using `portable-pty` crate.

### Protocol

gRPC schema at `proto/loopflow/control/v1/control.proto`. Key types: Wave, Stimulus, Agent with status enums.

---

## Docker Development

```bash
docker compose -f rust/lfd/docker-compose.yml up --build
```

---

## Tests

**loopflow-engine**: Flow parsing, tick execution, token counting

**lfd**: Store trait tests (SQLite + Postgres via testcontainers)
