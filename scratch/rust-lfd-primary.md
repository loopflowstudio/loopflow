# lfd as Primary Execution Path

## Problem

lfd has the infrastructure (gRPC, stores, loops, scheduler) but does not execute flows. loopflow-engine has `tick_flow()` and agent integration, but lfd doesn't use it. We're paying complexity twice:

- loopflow-engine has `runtime.rs` with its own `RunStore` trait and `tick_flow()` that spawns agents
- lfd has a richer store, scheduler, PTY sessions, but no execution

Result: neither works end-to-end.

## Component Diagram

```
┌─────────────────────────────────────────────────────────────────────────────┐
│ lfd                                                                         │
│                                                                             │
│  Triggers                 WaveExecutor.execute(run_id)                      │
│  ┌─────────────┐          ┌───────────────────────────────────────────────┐ │
│  │ loop (5s)   │─────────▶│                                               │ │
│  │ watch (30s) │          │  loop {                                       │ │
│  │ cron        │          │      action = next_action(flow, step_index)   │ │
│  │ RunWave RPC │          │                    │                          │ │
│  └─────────────┘          │                    ▼                          │ │
│                           │      match action {                           │ │
│                           │          RunStep { step } => {                │ │
│                           │              prompt = gather + format         │ │
│                           │              cmd = build_agent_command()      │ │
│                           │              spawn_and_stream(cmd)  ──────────│─│──▶ claude/codex
│                           │              step_index += 1                  │ │
│                           │          }                                    │ │
│                           │          WaitInteractive => break (PTY later) │ │
│                           │          Fork => create worktrees, recurse    │ │
│                           │          Complete => break                    │ │
│                           │      }                                        │ │
│                           │      store.update_run()                       │ │
│                           │  }                                            │ │
│                           └───────────────────────────────────────────────┘ │
│                                              │                              │
│  Store                                       │                              │
│  ┌───────────────────────────────────────────┼────────────────────────────┐ │
│  │  waves ──1:N──▶ wave_runs ──1:N──▶ agents ◀────────────────────────────┘ │
│  │  (config)       (execution)        (steps)                              │ │
│  └─────────────────────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────────────────┘
                          │
                          │ pure function calls
                          ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│ loopflow-engine (library) - NO state, NO spawning                           │
│                                                                             │
│  next_action(flow, step_index) ──▶ FlowAction::RunStep { step }            │
│                                    FlowAction::Fork { branches }            │
│                                    FlowAction::Complete                     │
│                                                                             │
│  gather_context(opts) ──▶ PromptComponents                                  │
│  format_prompt(components) ──▶ String                                       │
│  build_agent_command(model, prompt) ──▶ Vec<String>                         │
│                                                                             │
│  git: rebase, push, land, pr_create, sync_main                              │
│  worktree: create, remove, list                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

## Approach: FlowExecutor + WaveExecutor

Split execution into two clean layers:

| Layer | Location | Responsibility |
|-------|----------|----------------|
| **FlowExecutor** | loopflow-engine (library) | Stateless: parse flows, determine next action, gather context, format prompts |
| **WaveExecutor** | lfd | Stateful: own agent lifecycle, manage persistence, handle PTY, stream output |

### loopflow-engine (library)

**Pure functions, no side effects:**

```rust
// What's the next action for this flow?
pub fn next_action(flow: &Flow, step_index: usize) -> FlowAction

pub enum FlowAction {
    RunStep { step: Step },
    WaitInteractive { step: Step },
    Fork { branches: Vec<Step> },
    Choose { prompt: String, options: HashMap<String, Vec<FlowItem>> },
    Complete,
}

// Build the prompt for a step
pub fn gather_context(opts: &GatherContextOpts) -> PromptComponents
pub fn format_prompt(components: &PromptComponents) -> String

// Build agent CLI command (not spawn)
pub fn build_agent_command(model: &str, prompt: &str, config: &LaunchConfig) -> Vec<String>
```

**What stays:**
- `flow.rs` - flow parsing, `next_action()`
- `prompt.rs` - context gathering
- `agent.rs` - command building (not spawning)
- `git.rs` - git operations
- `config.rs`, `worktree.rs`, `naming.rs`

**What goes:**
- `runtime.rs` - delete entirely (logic moves to `flow.rs`, loop moves to lfd)
- `store.rs` - lfd owns persistence
- `bin/lf-engine.rs` - Python uses PyO3 directly
- `StepRunner` trait - no abstraction needed, lfd spawns directly

### WaveExecutor (lfd)

**Owns the loop** - persistence, spawning, streaming. No traits, just direct execution:

```rust
// lfd/src/executor.rs

pub struct WaveExecutor {
    store: SharedStore,
    scheduler: Arc<Scheduler>,
}

impl WaveExecutor {
    /// Execute a wave run to completion (or until interactive pause)
    pub async fn execute(&self, run_id: &LfdId) -> Result<()> {
        let mut run = self.store.get_run(run_id)?;
        let wave = self.store.get_wave(&run.wave_id)?;
        let flow = load_flow(&wave.flow, &wave.repo)?;

        loop {
            // loopflow-engine says what's next
            match next_action(&flow, run.step_index) {
                FlowAction::RunStep { step } => {
                    // lfd does the work directly
                    let prompt = self.build_prompt(&wave, &run, &step)?;
                    let cmd = build_agent_command(&step.model, &prompt, &config);

                    let agent = Agent::new(&run, &step);
                    self.store.create_agent(&agent)?;

                    let exit_code = self.spawn_and_stream(cmd, &run.worktree).await?;

                    self.store.end_agent(&agent.id, exit_code)?;

                    if exit_code == 0 {
                        run.step_index += 1;
                        self.store.update_run(&run)?;
                    } else {
                        run.status = WaveRunStatus::Failed;
                        self.store.update_run(&run)?;
                        return Ok(());
                    }
                }
                FlowAction::WaitInteractive { step } => {
                    let agent = Agent::waiting(&run, &step);
                    self.store.create_agent(&agent)?;
                    run.status = WaveRunStatus::Waiting;
                    self.store.update_run(&run)?;
                    return Ok(()); // ConnectWave resumes via PTY
                }
                FlowAction::Fork { branches } => {
                    // Create worktrees, execute branches, merge
                }
                FlowAction::Complete => {
                    run.status = WaveRunStatus::Completed;
                    run.ended_at = Some(now());
                    self.store.update_run(&run)?;
                    return Ok(());
                }
            }
        }
    }

    async fn spawn_and_stream(&self, cmd: Vec<String>, cwd: &Path) -> Result<i32> {
        // Spawn process, stream to broadcast channel, wait
    }
}
```

### What this enables

1. **lf CLI** uses loopflow-engine directly (no daemon required):
   - `lf run step` → gather_context + format_prompt + spawn agent locally
   - `lf ops rebase` → git operations
   - No waves, no persistence, just run and exit

2. **lfd** uses loopflow-engine for flow logic, owns wave execution:
   - Background loops call `executor.execute(run_id)`
   - `RunWave` RPC creates run and starts execution
   - `ConnectWave` → Concerto connects via PTY → Concerto sends session complete → lfd updates state

3. **No duplicate stores** - lfd's store is the only persistence layer

### ConnectWave flow (via Concerto)

```
lfd: WaveRun(Waiting), Agent(Waiting)
         │
         │ Concerto calls ConnectWave RPC
         ▼
lfd: returns { worktree, step, agent_id }
         │
         │ Concerto opens PTY, user works
         ▼
Concerto: session completes (user exits or step done)
         │
         │ Concerto calls EndAgent RPC with status
         ▼
lfd: Agent(Completed), WaveRun.step_index++, WaveRun(Running)
         │
         │ executor.execute() continues from new step_index
         ▼
```

`lf` CLI has no notion of wave connection - it just runs steps directly.

### lf CLI direct execution

No daemon required. Simple blocking execution:

```rust
// lf/src/run.rs

pub fn run_step(step_name: &str, worktree: &Path, directions: &[String]) -> Result<i32> {
    let step = load_step(step_name, worktree)?;
    let config = load_config(worktree)?;

    let opts = GatherContextOpts {
        repo_root: worktree.to_path_buf(),
        step: Some(step_name.to_string()),
        directions: directions.to_vec(),
        ..Default::default()
    };

    let components = gather_context(&opts)?;
    let prompt = format_prompt(&components);
    let cmd = build_agent_command(&step.model.unwrap_or(config.agent_model), &prompt, &config);

    // Spawn and block - no persistence, no streaming infrastructure
    let status = Command::new(&cmd[0])
        .args(&cmd[1..])
        .current_dir(worktree)
        .status()?;

    Ok(status.code().unwrap_or(1))
}
```

No waves, no WaveRun, no agents table - just run and exit.

## Fork execution

Fork runs branches in parallel, respecting scheduler slots. Atomic semantics: on any failure or crash, wipe fork state, mark run as failed, alert user.

### Fork model

```yaml
# .lf/flows/parallel-review.yaml
- fork:
    branches:
      - step: security-review
      - step: perf-review
      - step: ux-review
    synthesize: merge-reviews
```

### Fork data model

```
WaveRun (step_index = 2, status = Running)
    │
    └──▶ ForkRun[] (one per branch)
           ├── ForkRun { branch_index: 0, status: Running,   worktree: /wt/fork-0 }
           ├── ForkRun { branch_index: 1, status: Running,   worktree: /wt/fork-1 }
           └── ForkRun { branch_index: 2, status: Pending,   worktree: /wt/fork-2 }
                                                  ▲
                                                  └── waiting for scheduler slot
```

### Fork execution flow

```rust
FlowAction::Fork { branches, synthesize } => {
    // 1. Setup: create all worktrees and ForkRuns upfront
    let fork_runs = self.setup_fork(&run, &branches)?;

    // 2. Execute branches in parallel, respecting scheduler
    let (tx, mut rx) = mpsc::channel(branches.len());
    let mut handles = vec![];
    let mut running = 0;
    let mut pending: VecDeque<_> = fork_runs.iter().collect();

    while running > 0 || !pending.is_empty() {
        // Start branches up to available slots
        while !pending.is_empty() && self.scheduler.try_acquire() {
            let fork_run = pending.pop_front().unwrap();
            running += 1;

            let tx = tx.clone();
            let step = branches[fork_run.branch_index].clone();
            let worktree = fork_run.worktree.clone();
            let branch_idx = fork_run.branch_index;

            let handle = tokio::spawn(async move {
                let result = self.run_branch(&step, &worktree).await;
                tx.send((branch_idx, result)).await;
            });
            handles.push(handle);
        }

        // Wait for any branch to complete
        if let Some((branch_idx, result)) = rx.recv().await {
            running -= 1;
            self.scheduler.release();

            if result.is_err() || result.unwrap() != 0 {
                // Branch failed - kill running branches, wipe everything
                self.cancel_running_branches(&handles)?;
                self.cleanup_fork(&run, &fork_runs)?;
                run.status = Failed;
                run.error = Some(format!("fork branch {} failed", branch_idx));
                self.store.update_run(&run)?;
                return Ok(());
            }
        }
    }

    // 3. All branches complete - run synthesize step
    if let Some(synth_step) = synthesize {
        // Synthesize runs in main worktree, can read all fork worktrees
        let exit_code = self.run_step(&synth_step, &run.worktree).await?;
        if exit_code != 0 {
            self.cleanup_fork(&run, &fork_runs)?;
            run.status = Failed;
            self.store.update_run(&run)?;
            return Ok(());
        }
    }

    // 4. Success - cleanup and advance
    self.cleanup_fork(&run, &fork_runs)?;
    run.step_index += 1;
    self.store.update_run(&run)?;
}

fn cleanup_fork(&self, run: &WaveRun, fork_runs: &[ForkRun]) -> Result<()> {
    for fork_run in fork_runs {
        let _ = remove_worktree(&fork_run.worktree);  // best effort
    }
    self.store.delete_fork_runs(&run.id)?;
    Ok(())
}
```

### Recovery strategy: limited retry, then fail

Fork is atomic. If anything goes wrong:

1. **On branch failure:** cancel running branches, cleanup fork state
2. **Retry up to 3 times** (configurable) for transient failures
3. **After max retries:** mark WaveRun as Failed, alert user
4. **On crash recovery:** counts as one failure, cleanup and retry or fail

```rust
// WaveRun tracks fork attempts
pub struct WaveRun {
    // ...
    pub fork_attempts: u32,  // reset to 0 when fork succeeds
}

// On fork failure:
run.fork_attempts += 1;
self.cleanup_fork(&run, &fork_runs)?;

if run.fork_attempts < MAX_FORK_RETRIES {
    // Will retry on next tick
    self.store.update_run(&run)?;
} else {
    run.status = Failed;
    run.error = Some(format!("fork failed after {} attempts", run.fork_attempts));
    self.store.update_run(&run)?;
    self.notify_failure(&run)?;
}
```

Most failures need user intervention, but a few retries handles transient issues.

### Output streaming

Each branch streams to its own channel:

```rust
// StreamOutput RPC can filter by agent_id or get interleaved
message StreamOutputRequest {
    string wave_run_id = 1;
    optional string agent_id = 2;  // filter to specific branch
}
```

Concerto can show parallel branches in split view or tabs.

### Fork constraints

- No interactive steps in fork branches (would block parallelism)
- All branches must succeed for synthesize to run
- Synthesize step can read all fork worktrees before cleanup
- Max parallelism bounded by scheduler slots

## Choose execution

Choose needs LLM to pick a branch based on a prompt.

```yaml
- choose:
    prompt: "Based on the PR feedback, what should we do?"
    options:
      refactor: [step: major-refactor]
      tweak: [step: minor-fixes]
      done: []  # empty = skip to next
```

```rust
FlowAction::Choose { prompt, options } => {
    // Spawn a "choice agent" that returns the selected option
    let choice_prompt = format!("{}\n\nOptions: {}\n\nRespond with just the option name.",
        prompt, options.keys().join(", "));
    let cmd = build_agent_command("claude:haiku", &choice_prompt);
    let output = self.spawn_and_capture(cmd).await?;

    let selected = output.trim();
    let branch_steps = options.get(selected)
        .ok_or_else(|| anyhow!("invalid choice: {}", selected))?;

    // Execute selected branch inline
    for step in branch_steps {
        // ... run step ...
    }

    run.step_index += 1;
}
```

## Removed: LoopUntilEmpty

LoopUntilEmpty was complex and rarely used. Removed from scope. If needed later, can be a separate flow or wave-level iteration.

## Store changes

Add `wave_runs` table, simplify `waves`:

```sql
-- waves: config only (remove execution state)
CREATE TABLE waves (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    repo TEXT NOT NULL,
    flow TEXT NOT NULL,
    direction TEXT NOT NULL,  -- JSON array
    area TEXT NOT NULL,       -- JSON array
    paused BOOLEAN DEFAULT FALSE,
    created_at INTEGER
);

-- NEW: wave_runs tracks execution
CREATE TABLE wave_runs (
    id TEXT PRIMARY KEY,
    wave_id TEXT NOT NULL REFERENCES waves(id),
    iteration INTEGER NOT NULL,
    step_index INTEGER DEFAULT 0,
    status INTEGER DEFAULT 0,  -- Pending, Running, Waiting, Completed, Failed
    worktree TEXT,
    branch TEXT,
    started_at INTEGER,
    ended_at INTEGER,
    error TEXT
);

-- agents: now references wave_run_id
CREATE TABLE agents (
    id TEXT PRIMARY KEY,
    wave_run_id TEXT REFERENCES wave_runs(id),
    step TEXT NOT NULL,
    status INTEGER DEFAULT 0,
    pid INTEGER,
    started_at INTEGER,
    ended_at INTEGER,
    exit_code INTEGER
);
```

Remove from Wave proto:
- `status`, `step_index`, `iteration` (now on WaveRun)
- `consecutive_failures`, `pid` (derive from recent runs)
- `worktree`, `branch` (now on WaveRun)

## Deletions

| Item | Reason |
|------|--------|
| `loopflow-engine/src/runtime.rs` | `next_action()` moves to `flow.rs`, loop moves to lfd |
| `loopflow-engine/src/store.rs` | lfd owns persistence |
| `loopflow-engine/src/bin/lf-engine.rs` | Python uses PyO3 directly |
| `CommandStepRunner` | Weird circular thing that shelled out to `lf` |
| `LoopUntilEmpty` flow construct | Too complex, rarely used |

Note: Keep `StepRunner` trait in lfd for testing (MockRunner).

## New structure

```
rust/loopflow-engine/src/
├── lib.rs
├── flow.rs           # Flow parsing + next_action() + FlowAction enum
├── prompt.rs         # Context gathering
├── agent.rs          # Command building (not spawning)
├── git.rs            # Git operations
├── config.rs         # Config loading
├── worktree.rs       # Worktree operations
├── naming.rs         # Branch naming
├── builtins.rs       # Built-in steps
├── error.rs          # Error types
└── python.rs         # PyO3 bindings

rust/lfd/src/
├── main.rs
├── server.rs
├── executor.rs       # NEW: WaveExecutor + AgentRunner
├── scheduler.rs
├── sessions.rs       # PTY
├── store/            # Only persistence layer
└── loops/            # Signal sources, call executor.execute()
```

## Data model: Wave → WaveRun → Agent

```
┌─────────────────────────────────────────────────────────────────────────────┐
│ PERSISTENT (lfd store)                                                      │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  Wave (config)                    Stimulus (triggers)                       │
│  ┌─────────────────────────┐      ┌─────────────────────────┐              │
│  │ id visionary-20260203   │──1:N─│ id                      │              │
│  │ name "visionary"        │      │ wave_id                 │              │
│  │ repo /code/myapp        │      │ kind: Loop|Watch|Cron   │              │
│  │ flow "ship"             │      │ cron "0 9 * * *"        │              │
│  │ directions [quality]    │      │ enabled true            │              │
│  │ areas [src/]            │      └─────────────────────────┘              │
│  │ paused false            │                                                │
│  └───────────┬─────────────┘                                                │
│              │                                                              │
│              │ 1:N (triggers create runs)                                   │
│              ▼                                                              │
│  WaveRun (execution instance)                                               │
│  ┌─────────────────────────┐                                                │
│  │ id run-abc123           │                                                │
│  │ wave_id                 │                                                │
│  │ iteration 3             │                                                │
│  │ step_index 2            │◀─── flow progress                              │
│  │ status Running          │                                                │
│  │ worktree /wt/vis-3      │                                                │
│  │ branch vis-20260203-3   │                                                │
│  │ started_at, ended_at    │                                                │
│  │ error                   │                                                │
│  └───────────┬─────────────┘                                                │
│              │                                                              │
│              │ 1:N (each step spawns agent)                                 │
│              ▼                                                              │
│  Agent (step execution)                                                     │
│  ┌─────────────────────────┐                                                │
│  │ id agent-xyz            │                                                │
│  │ wave_run_id             │                                                │
│  │ step "implement"        │                                                │
│  │ status Completed        │                                                │
│  │ pid 12345               │                                                │
│  │ started_at, ended_at    │                                                │
│  │ exit_code 0             │                                                │
│  └─────────────────────────┘                                                │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────────────────┐
│ EPHEMERAL (loopflow-engine, loaded from .lf/)                               │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  Flow (parsed from .lf/flows/ship.yaml)                                     │
│  ┌─────────────────────────────────────────────────────────────────┐       │
│  │ name "ship"                                                     │       │
│  │ items [                                                         │       │
│  │   Step { name: "plan", interactive: false }      ◀─ step_index 0│       │
│  │   Step { name: "implement", interactive: false } ◀─ step_index 1│       │
│  │   Step { name: "review", interactive: true }     ◀─ step_index 2│       │
│  │   Fork { branches: [...], synthesize: "merge" }  ◀─ step_index 3│       │
│  │ ]                                                               │       │
│  └─────────────────────────────────────────────────────────────────┘       │
│                                                                             │
│  Step (parsed from .lf/steps/implement.md)                                  │
│  ┌─────────────────────────────────────────────────────────────────┐       │
│  │ name "implement"                                                │       │
│  │ model "claude:sonnet"                                           │       │
│  │ directions ["quality", "testing"]                               │       │
│  │ interactive false                                               │       │
│  │ content "# implement\n\nImplement the design..."                │       │
│  └─────────────────────────────────────────────────────────────────┘       │
│                                                                             │
│  Direction (parsed from .lf/directions/quality.md)                          │
│  ┌─────────────────────────────────────────────────────────────────┐       │
│  │ name "quality"                                                  │       │
│  │ content "Focus on code quality, write tests..."                 │       │
│  └─────────────────────────────────────────────────────────────────┘       │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

**Benefits:**
- Query historical runs: "show me last 10 runs of this wave"
- Clear separation: wave config doesn't change when execution happens
- Atomic state: each run is its own entity with clear lifecycle
- No more `consecutive_failures` on Wave - derive from recent WaveRuns

**Status flow:**
```
Wave.paused == false && stimulus fires
    → create WaveRun(status: Pending)
    → WaveRun(status: Running)
        → Agent(status: Running) for each step
        → Agent(status: Completed/Failed)
    → WaveRun(status: Completed/Failed)
```

**Concurrent runs:** A wave can only have one active WaveRun at a time (v1). If stimulus fires while a run is active:
- Loop/Cron: ignored (run already in progress)
- Watch: coalesced into pending activation (existing behavior)
- RunWave RPC: returns error "wave already running"

## Testing

Keep `StepRunner` trait for testing only:

```rust
// In lfd
pub trait StepRunner: Send + Sync {
    fn run(&self, step: &Step, worktree: &Path, directions: &[String]) -> Result<StepResult>;
}

// Production: spawns real agents
pub struct AgentRunner;
impl StepRunner for AgentRunner { ... }

// Test: returns canned results
pub struct MockRunner {
    results: HashMap<String, StepResult>,  // step name -> result
}
impl StepRunner for MockRunner { ... }
```

This lets us test fork completion, choose branching, error handling without spawning real agents.

## Key decisions

- **loopflow-engine is stateless** - no store trait, no process management
- **lfd owns execution** - WaveExecutor manages agent lifecycle
- **lf CLI runs directly** - no daemon required for `lf run step`
- **Delete lf-engine binary** - Python uses PyO3 bindings directly
- **Single store** - lfd's store is the source of truth
- **Wave → WaveRun → Agent** - separate config from execution state
- **Remove LoopUntilEmpty** - too complex, rarely used

## Scope

In scope:
- Add `next_action()` and `FlowAction` to loopflow-engine/flow.rs
- Create WaveExecutor in lfd
- Wire RunWave/ConnectWave/StreamOutput to WaveExecutor
- Update loop/watch/cron to call executor.execute()
- Delete runtime.rs, store.rs, lf-engine binary
- Proto changes:
  - Add `WaveRun` message
  - Add `ListWaveRuns`, `GetWaveRun` RPCs
  - Remove execution fields from `Wave` (status, step_index, etc.)
  - Update `Agent` to reference `wave_run_id`
- Store migration: move execution state from waves to wave_runs table

Out of scope:
- Container/K8s executors (phase 2)
- Auth/TLS (phase 2)
- Service install (separate doc)

## Done when

- `RunWave` executes a flow end-to-end through WaveExecutor
- Interactive steps pause with WaveRunStatus::Waiting, resume via ConnectWave
- `StreamOutput` streams agent stdout/stderr in real time
- loop/watch/cron triggers call `executor.execute()`
- Fork execution works: parallel branches, fail-fast on error, cleanup on crash
- lf-engine binary deleted, Python uses PyO3
- Proto has WaveRun message, Wave simplified
- Store migrated to wave_runs table
- `cargo test -p lfd` passes for execution + fork + choose tests
