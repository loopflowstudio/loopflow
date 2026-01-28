# Session Connect Rust

Rust implementation of the lfd daemon's connectivity layer.

## What to build

A Rust binary (`lfd-rs`) that provides the same HTTP API as the current Python `lfd`, with SQLite persistence and event streaming. Execution shells out to `lf` commands—Rust handles infrastructure, Python handles prompt/context logic.

Key capability: **Session connect for interactive steps.** Flows can pause at interactive steps (`WaveStatus.WAITING`), and users can "connect" to resume them. This requires tick-based state machine execution that Rust must orchestrate.

## Why

From rust-lfd.md: "A daemon should be rock-solid. Rust's memory safety and lack of GC pauses could improve reliability for something that runs 24/7."

The current Python daemon works but has tradeoffs:
- Slower startup (matters for launchd restarts)
- Higher idle memory footprint
- asyncio complexity for long-running server

## Architecture

```
lfd-rs (Rust)
├── HTTP server (axum)
│   ├── /status, /health
│   ├── /worktrees?repo=PATH
│   ├── /flows?repo=PATH
│   └── /waves/* (CRUD)
├── Unix socket server (tokio)
│   ├── JSON-over-newline protocol
│   └── Event subscriptions
├── SQLite (rusqlite)
│   └── ~/.lf/lfd.db (same schema)
└── Execution: shell to lf
```

Concerto (Swift) talks HTTP. The socket server is for CLI subscriptions and fire-and-forget notifications from `lf` runs.

## Data structures

```rust
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Wave {
    pub id: String,
    pub name: String,
    pub repo: String,
    pub flow: String,
    pub direction: Option<Vec<String>>,
    pub area: Option<Vec<String>>,
    pub stimulus: Stimulus,
    pub paused: bool,
    pub status: WaveStatus,
    pub iteration: i32,
    pub worktree: Option<String>,
    pub branch: Option<String>,
    pub pr_limit: i32,
    pub merge_mode: MergeMode,
    pub pid: Option<i32>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Stimulus {
    pub kind: String,  // "once", "loop", "watch", "cron"
    pub cron: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WaveStatus {
    Idle,
    Running,
    Waiting,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MergeMode {
    Pr,
    Land,
}
```

Protocol types (matching Python):

```rust
#[derive(Debug, Serialize, Deserialize)]
pub struct Request {
    pub method: String,
    pub params: serde_json::Value,
    pub id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Response {
    pub ok: bool,
    pub result: Option<serde_json::Value>,
    pub error: Option<String>,
    pub id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Event {
    pub event: String,
    pub data: serde_json::Value,
}
```

## Interactive step flow (session connect)

Flows can contain interactive steps (design, explore, refine). When a flow hits an interactive step:

1. Daemon pauses flow, sets `WaveStatus::Waiting`
2. Creates `StepRun` with `status: waiting`
3. Broadcasts `wave.waiting` event with step details
4. User sees wave is waiting in Concerto or CLI
5. User runs `lfd connect <wave-id>` or clicks "Connect" in Concerto
6. Connect opens terminal with interactive step prompt
7. When step completes, `lfd` notifies daemon
8. Daemon advances `FlowRun.step_index`, continues flow

```
Flow: design-and-ship
  ├─ design (interactive) ──────────────────────┐
  ├─ implement (auto)                           │
  ├─ reduce (auto)        [tick_flow state      │
  └─ polish (auto)         machine pauses here] │
                                                │
                           ┌────────────────────┘
                           │
User connects ─────────────▼
  lfd connect <wave-id>
  └─ Opens terminal with assembled prompt
  └─ User completes design session
  └─ Exit signals completion

Daemon receives step_run.end ─────────────────▶ Advances step_index
                                               Continues auto steps
```

### Data structures for session state

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlowRun {
    pub id: String,
    pub wave_id: Option<String>,
    pub flow: String,
    pub direction: Vec<String>,
    pub area: Vec<String>,
    pub repo: String,
    pub status: FlowRunStatus,
    pub step_index: i32,  // Position in flow for tick-based execution
    pub worktree: Option<String>,
    pub branch: Option<String>,
    pub current_step: Option<String>,
    pub error: Option<String>,
    pub started_at: Option<DateTime<Utc>>,
    pub ended_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepRun {
    pub id: String,
    pub step: String,
    pub repo: String,
    pub worktree: String,
    pub flow_run_id: Option<String>,
    pub wave_id: Option<String>,
    pub status: StepRunStatus,  // running, waiting, completed, failed
    pub run_mode: String,       // "auto" or "interactive"
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
}
```

### Connect API

```rust
// HTTP endpoint for session connect
async fn connect_wave(wave_id: Path<String>) -> Json<ConnectResponse>
// Returns: { worktree, step, step_run_id, prompt_file }

// Socket methods
"step_runs.start" -> Records step start, broadcasts session.started
"step_runs.end"   -> Records step end, broadcasts session.ended, triggers tick_flow
```

### Tick-based execution

```rust
enum TickResult {
    StepComplete,        // Continue ticking
    FlowComplete,        // Flow finished
    WaitingInteractive,  // Paused for user
    StepFailed,          // Step errored
}

fn tick_flow(flow_run_id: &str) -> TickResult {
    // 1. Load flow_run from DB
    // 2. Check if next step is interactive
    // 3. If interactive: create WAITING StepRun, return WaitingInteractive
    // 4. If auto: shell to `lf --step <step> --worktree <path>`
    // 5. Advance step_index, return StepComplete
}
```

## Key functions

```rust
// HTTP handlers
async fn get_status() -> Json<LFDResponse>
async fn get_health() -> Json<LFDResponse>
async fn list_worktrees(repo: Query<String>) -> Json<LFDResponse>
async fn list_waves(repo: Query<String>) -> Json<LFDResponse>
async fn create_wave(repo: Query<String>, body: Json<CreateWaveRequest>) -> Json<LFDResponse>
async fn update_wave(wave_id: Path<String>, body: Json<UpdateWaveRequest>) -> Json<LFDResponse>
async fn delete_wave(wave_id: Path<String>) -> Json<LFDResponse>
async fn run_wave(wave_id: Path<String>) -> Json<LFDResponse>
async fn stop_wave(wave_id: Path<String>) -> Json<LFDResponse>
async fn connect_wave(wave_id: Path<String>) -> Json<ConnectResponse>  // Session connect

// Socket handlers
async fn handle_subscribe(params: Value, tx: Sender<Event>) -> Response
async fn handle_notify(params: Value) -> Response
async fn handle_step_runs_start(params: Value) -> Response  // Track session lifecycle
async fn handle_step_runs_end(params: Value) -> Response    // Triggers tick_flow
async fn broadcast(event: Event, subscribers: &Subscribers)

// Flow execution
fn tick_flow(flow_run_id: &str) -> TickResult  // State machine executor

// Database
fn get_db() -> Connection
fn list_waves_db(repo: &str) -> Vec<Wave>
fn get_wave_db(id: &str) -> Option<Wave>
fn save_wave_db(wave: &Wave)
fn delete_wave_db(id: &str) -> bool
fn get_flow_run_db(id: &str) -> Option<FlowRun>
fn update_flow_run_index(id: &str, index: i32)
fn get_waiting_step_run(wave_id: &str) -> Option<StepRun>

// Execution (shells to lf)
async fn start_wave_process(wave: &Wave) -> Result<Child>
async fn stop_wave_process(wave: &Wave) -> Result<()>
async fn run_step_auto(step: &str, worktree: &Path) -> Result<i32>  // lf --step
```

## Worktree state

The Python daemon uses `WorktreeStateService` to enrich wave responses with git status. For Rust:

```rust
// Shell to git commands directly
fn get_worktree_state(repo: &Path, branch: &str) -> WorktreeState {
    // git status --porcelain
    // git rev-list --left-right HEAD...origin/main
    // gh pr view (for CI state)
}
```

Or simpler: shell to `lf` with a new command:

```bash
lf worktree-state --repo PATH --branch BRANCH --json
```

This keeps git/gh logic in Python, Rust just calls it.

## Constraints

**Protocol compatibility is required.** Concerto expects exact HTTP response format:
```json
{"ok": true, "result": {...}, "version": "0.7.0"}
```

**Database schema is shared.** Rust and Python must read/write the same `~/.lf/lfd.db`. Use SQLite WAL mode for concurrent access.

**Single instance enforcement.** Check HTTP health endpoint before starting; fail if another lfd is running.

**Signal handling.** SIGTERM/SIGINT → graceful shutdown → WAL checkpoint.

## Migration path

1. Build lfd-rs with core HTTP endpoints
2. Add feature flag to `lfd serve`: `--rust` or env `LFD_RUST=1`
3. When flag is set, spawn lfd-rs instead of Python server
4. Test with Concerto: worktrees, waves CRUD, run/stop
5. Once stable, make Rust the default
6. Eventually remove Python daemon code

Version decoupling: lfd-rs can have its own version. Protocol version (in /health response) is the compatibility contract.

## Done when

```bash
# Build
cargo build --release -p lfd

# Start Rust daemon
./target/release/lfd serve

# From another terminal
curl http://127.0.0.1:8765/health
# {"ok":true,"result":{"version":"0.1.0","uptime_seconds":5,...}}

curl "http://127.0.0.1:8765/worktrees?repo=/path/to/repo"
# {"ok":true,"result":{"worktrees":[...]}}

curl "http://127.0.0.1:8765/waves?repo=/path/to/repo"
# {"ok":true,"result":{"waves":[...]}}

# Concerto connects and shows worktrees/waves
```

**Session connect test:**
```bash
# Start a wave with interactive flow
lfd run mywave --flow design-and-ship

# Wave should pause at design step
lfd status mywave
# status: waiting, step: design

# Connect to interactive step
lfd connect mywave
# Opens terminal with design prompt...
# Complete the session, exit

# Flow continues automatically
lfd status mywave
# status: running, step: implement
```

## File structure

```
rust/
├── Cargo.toml
├── lfd/
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs
│       ├── http.rs      # axum handlers
│       ├── socket.rs    # tokio unix socket
│       ├── db.rs        # rusqlite wrapper
│       ├── models.rs    # Wave, StepRun, etc.
│       └── process.rs   # subprocess management
```

## Dependencies

```toml
[dependencies]
tokio = { version = "1", features = ["full"] }
axum = "0.7"
tower-http = { version = "0.5", features = ["cors"] }
rusqlite = { version = "0.31", features = ["bundled"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
chrono = { version = "0.4", features = ["serde"] }
uuid = { version = "1", features = ["v4"] }
```

## Open questions

1. **Worktree enrichment**: Shell to `git` directly or add `lf worktree-state` command?

2. **Flows endpoint**: Currently calls Python `list_flows()`. Shell to `lf flows --json`?

3. **PR polling**: Python daemon polls GitHub for CI status. Keep in Rust or shell out?

4. **Terminal attachment for connect**: How does `lfd connect` attach the user's terminal to the step? Options:
   - Shell to `lf --step <step> --worktree <path>` with exec
   - Return prompt file path, let CLI tool handle terminal
   - WebSocket for Concerto-based terminal

Best guess: Start with shelling out for everything complex. Add native implementations later if performance matters.

## Relation to rust-lfd.md

The existing `reports/rust-lfd.md` discusses whether to port lfd to Rust but doesn't cover:
- **Tick-based flow execution** (step_index state machine)
- **Session connect** (WaitingInteractive → user attaches → continues)
- **StepRun lifecycle** for interactive steps

Suggest adding to rust-lfd.md:

```markdown
## Interactive Step Handling (new capability)

Flows with interactive steps require tick-based execution:
1. FlowRun tracks `step_index` position
2. At interactive step: pause, set WAITING, emit event
3. User connects via CLI/Concerto
4. On completion: advance step_index, continue

This is orchestration logic Rust must own—can't shell out because state
must persist across user sessions. The daemon becomes a state machine
coordinator, not just a process launcher.
```

This changes the Rust daemon from "process launcher that shells to lf" to "state machine that coordinates tick-based execution with user interaction."
