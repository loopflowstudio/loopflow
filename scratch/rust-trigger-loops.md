# Trigger Evaluation Loops

Implement the scheduling loops that drive wave execution in the Rust daemon.

## Problem

The Rust daemon has gRPC endpoints and a store but no active scheduling. Waves can be created and their status changed, but nothing actually runs them. The daemon is passive—a CRUD server for wave metadata.

Users benefit from: waves that execute automatically based on their stimulus (loop, watch, cron), stuck run recovery, and predictable scheduling behavior matching the Python daemon.

Why now: The store, proto types, and gRPC endpoints exist. The control plane skeleton is complete. The loops are the engine that drives it.

## Approach

Add four background tokio tasks to the `Scheduler`, started via `start_loops()` and coordinated via `CancellationToken` for graceful shutdown.

**Loop architecture:**

| Loop | Interval | Responsibility |
|------|----------|----------------|
| `loop_ticker` | 5s | Tick waves with `StimulusKind::Loop` in `Running` status |
| `watch_poller` | 30s | Check waves with `StimulusKind::Watch` for git changes |
| `cron_poller` | 30s | Evaluate cron schedules for `StimulusKind::Cron` waves |
| `recovery_loop` | 60s | Find step runs stuck >4h, terminate and retry/fail |

Each loop:
1. Queries store for relevant waves
2. Evaluates trigger condition
3. Acquires slot via semaphore (loop_ticker) or queues activation
4. Calls `lf-core::tick_flow` or marks wave for activation
5. Updates store with result
6. Respects `CancellationToken` for shutdown

**Key invariant:** A wave in `Waiting` status (interactive step) does not tick until `EndStepRun` succeeds. Loops skip waves where `status == Waiting`.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Single unified poll loop | Simpler code | Different concerns have different intervals; conflating them wastes cycles or delays checks |
| Spawn task per wave | Natural isolation | Thundering herd on startup; harder to reason about concurrency limits |
| Pure event-driven | No polling overhead | Watch/cron are inherently external state requiring polling; can't receive events from git |
| Channel-based coordination | Decoupled concerns | Adds complexity without benefit; loops already share store via Arc |

## Key decisions

**Separate loops per concern.** Per daemon-service design doc: events for internal state, polling for external state. Loop/watch/cron/recovery have different intervals and semantics.

**Store-first state machine.** Loops read from and write to store. No in-memory wave state that can diverge. If daemon crashes and restarts, it resumes from store.

**Slot acquisition in loop_ticker only.** Watch and cron pollers queue activations (increment `pending_activations`). Loop ticker acquires semaphore permits and actually ticks. This prevents watch/cron from bypassing concurrency limits.

**git2 for watch checking.** Shell out to `git fetch` then `git rev-parse` is fragile. Use `git2` crate for direct access. Fetch `origin/main`, compare SHA to `last_main_sha`, diff for matching paths in `area`.

**cron crate for schedule evaluation.** Use `cron` crate to parse expressions and find next occurrence. Compare against last run end time from store.

**Retry cap at wave level.** The `consecutive_failures` field on Wave tracks failures. 3 consecutive failures → wave enters `Error` status. Reset to 0 on success.

**tokio_util::CancellationToken for shutdown.** All loops check `token.is_cancelled()` in their select. On SIGTERM, cancel token, await all handles.

**Reuse Wave fields for flow state.** The Wave proto already has `iteration`, `worktree`, `branch`, `consecutive_failures`. Add `step_index` field to track position in flow. No separate FlowRun table needed for Stage 3.

## Scope

In scope:
- `Scheduler::start_loops()` returning `Vec<JoinHandle<()>>`
- Loop ticker: query waves, filter by stimulus/status, acquire slot, call tick, update store
- Watch poller: fetch git, compare SHA, check path matches, queue activation
- Cron poller: parse cron, compare to last run, queue activation
- Recovery loop: find stuck step runs, kill PID, mark failed, respect retry cap
- Add `step_index` field to Wave proto for flow position tracking
- Store methods: `list_waves_by_stimulus`, `increment_pending_activations`, `get_stuck_step_runs`
- Integration with lf-core `tick_flow` via store trait bridge

Out of scope:
- FlowRun as separate table (reuse Wave fields; Stage 5 can separate if needed)
- PR polling (deferred—not required for basic loop/watch/cron)
- Event streaming to clients (separate work)
- Subprocess worker isolation (Stage 6)

## Components

```
rust/lfd/src/
├── main.rs           # Add cancel token, spawn loops, await on shutdown
├── scheduler.rs      # Add start_loops() method
└── loops/
    ├── mod.rs        # Module exports
    ├── loop_ticker.rs
    ├── watch.rs
    ├── cron.rs
    └── recovery.rs
```

**scheduler.rs additions:**

```rust
impl Scheduler {
    pub fn start_loops(
        self: Arc<Self>,
        store: SharedStore,
        cancel: CancellationToken,
    ) -> Vec<JoinHandle<()>> {
        vec![
            loops::spawn_loop_ticker(self.clone(), store.clone(), cancel.clone()),
            loops::spawn_watch_poller(store.clone(), cancel.clone()),
            loops::spawn_cron_poller(store.clone(), cancel.clone()),
            loops::spawn_recovery_loop(store.clone(), cancel.clone()),
        ]
    }
}
```

**Store trait additions:**

```rust
// store/mod.rs
fn list_waves_by_stimulus(&self, kind: i32) -> StoreResult<Vec<Wave>>;
fn increment_pending_activations(&self, wave_id: &str) -> StoreResult<u32>;
fn get_stuck_step_runs(&self, older_than_secs: u64) -> StoreResult<Vec<StepRun>>;
```

**main.rs integration:**

```rust
let cancel = CancellationToken::new();
let loop_handles = scheduler.clone().start_loops(store.clone(), cancel.clone());

tokio::select! {
    result = grpc_task => { ... }
    result = http_task => { ... }
    _ = signal::ctrl_c() => {
        tracing::info!("shutdown signal received, draining loops...");
        cancel.cancel();
        for handle in loop_handles {
            let _ = handle.await;
        }
    }
}
```

## Loop ticker detail

```rust
// loops/loop_ticker.rs

pub fn spawn_loop_ticker(
    scheduler: Arc<Scheduler>,
    store: SharedStore,
    cancel: CancellationToken,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(5));
        loop {
            tokio::select! {
                _ = cancel.cancelled() => {
                    tracing::info!("loop_ticker shutting down");
                    break;
                }
                _ = interval.tick() => {
                    tick_loop_waves(&scheduler, &store).await;
                }
            }
        }
    })
}

async fn tick_loop_waves(scheduler: &Scheduler, store: &SharedStore) {
    let waves = match store.list_waves_by_stimulus(StimulusKind::Loop as i32) {
        Ok(w) => w,
        Err(e) => {
            tracing::error!("failed to list loop waves: {}", e);
            return;
        }
    };

    for wave in waves {
        if wave.paused || wave.status() != WaveStatus::Running {
            continue;
        }

        let (acquired, _) = scheduler.acquire(&wave.id).await;
        if !acquired {
            tracing::debug!(wave_id = %wave.id, "waiting for slot");
            continue;
        }

        let result = tick_wave(&wave, store).await;
        handle_tick_result(&wave, result, store);
        scheduler.release(&wave.id);
    }
}

fn handle_tick_result(wave: &Wave, result: Result<TickResult, CoreError>, store: &SharedStore) {
    match result {
        Ok(TickResult::StepComplete) => {
            // Reset failures on success, increment iteration
            let mut w = wave.clone();
            w.consecutive_failures = 0;
            let _ = store.update_wave(&w);
        }
        Ok(TickResult::FlowComplete) => {
            let mut w = wave.clone();
            w.status = WaveStatus::Idle as i32;
            w.iteration += 1;
            w.consecutive_failures = 0;
            let _ = store.update_wave(&w);
            tracing::info!(wave_id = %w.id, iteration = w.iteration, "flow complete");
        }
        Ok(TickResult::WaitingInteractive) => {
            let mut w = wave.clone();
            w.status = WaveStatus::Waiting as i32;
            let _ = store.update_wave(&w);
            tracing::info!(wave_id = %w.id, "waiting for interactive step");
        }
        Err(e) => {
            let mut w = wave.clone();
            w.consecutive_failures += 1;
            if w.consecutive_failures >= 3 {
                w.status = WaveStatus::Error as i32;
                tracing::error!(wave_id = %w.id, "entered error state after 3 failures");
            }
            let _ = store.update_wave(&w);
            tracing::warn!(wave_id = %w.id, error = %e, "tick failed");
        }
    }
}
```

## Watch poller detail

```rust
// loops/watch.rs

pub fn spawn_watch_poller(store: SharedStore, cancel: CancellationToken) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(30));
        loop {
            tokio::select! {
                _ = cancel.cancelled() => {
                    tracing::info!("watch_poller shutting down");
                    break;
                }
                _ = interval.tick() => {
                    check_watch_waves(&store);
                }
            }
        }
    })
}

fn check_watch_waves(store: &SharedStore) {
    let waves = match store.list_waves_by_stimulus(StimulusKind::Watch as i32) {
        Ok(w) => w,
        Err(e) => {
            tracing::error!("failed to list watch waves: {}", e);
            return;
        }
    };

    for wave in waves {
        if wave.paused {
            continue;
        }

        match check_watch_stimulus(&wave) {
            Ok(true) => {
                // Changes detected in area
                if wave.status() == WaveStatus::Running || wave.status() == WaveStatus::Waiting {
                    // Busy—queue activation
                    let _ = store.increment_pending_activations(&wave.id);
                    tracing::debug!(wave_id = %wave.id, "watch: queued activation");
                } else {
                    // Idle—start directly
                    let mut w = wave.clone();
                    w.status = WaveStatus::Running as i32;
                    let _ = store.update_wave(&w);
                    tracing::info!(wave_id = %wave.id, "watch: activated");
                }
            }
            Ok(false) => {
                // No change or change outside area
            }
            Err(e) => {
                tracing::warn!(wave_id = %wave.id, error = %e, "watch check failed");
            }
        }
    }
}

fn check_watch_stimulus(wave: &Wave) -> Result<bool, git2::Error> {
    let repo = Repository::open(&wave.repo)?;

    // Fetch origin/main
    let mut remote = repo.find_remote("origin")?;
    remote.fetch(&["main"], None, None)?;

    // Get current SHA
    let reference = repo.find_reference("refs/remotes/origin/main")?;
    let current_sha = reference.peel_to_commit()?.id().to_string();

    // Compare to stored SHA
    let last_sha = wave.last_main_sha.as_deref();
    if Some(current_sha.as_str()) == last_sha {
        return Ok(false);
    }

    // New SHA—check if changes touch area
    if let Some(prev) = last_sha {
        let prev_oid = git2::Oid::from_str(prev)?;
        let curr_oid = git2::Oid::from_str(&current_sha)?;

        let prev_commit = repo.find_commit(prev_oid)?;
        let curr_commit = repo.find_commit(curr_oid)?;

        let prev_tree = prev_commit.tree()?;
        let curr_tree = curr_commit.tree()?;

        let diff = repo.diff_tree_to_tree(Some(&prev_tree), Some(&curr_tree), None)?;

        let area_match = diff.deltas().any(|d| {
            let path = d.new_file().path().unwrap_or(Path::new(""));
            wave.area.iter().any(|a| path.starts_with(a))
        });

        if !area_match {
            // Update SHA but don't activate—changes outside area
            // (caller handles SHA update)
            return Ok(false);
        }
    }

    Ok(true)
}
```

## Cron poller detail

```rust
// loops/cron.rs

pub fn spawn_cron_poller(store: SharedStore, cancel: CancellationToken) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(30));
        loop {
            tokio::select! {
                _ = cancel.cancelled() => {
                    tracing::info!("cron_poller shutting down");
                    break;
                }
                _ = interval.tick() => {
                    check_cron_waves(&store);
                }
            }
        }
    })
}

fn check_cron_waves(store: &SharedStore) {
    let waves = match store.list_waves_by_stimulus(StimulusKind::Cron as i32) {
        Ok(w) => w,
        Err(e) => {
            tracing::error!("failed to list cron waves: {}", e);
            return;
        }
    };

    for wave in waves {
        if wave.paused {
            continue;
        }

        let cron_expr = match &wave.stimulus {
            Some(s) if !s.cron.is_empty() => &s.cron,
            _ => continue,
        };

        let last_run = get_last_run_end_time(store, &wave.id);

        if should_activate_cron(cron_expr, last_run) {
            if wave.status() == WaveStatus::Running || wave.status() == WaveStatus::Waiting {
                let _ = store.increment_pending_activations(&wave.id);
                tracing::debug!(wave_id = %wave.id, "cron: queued activation");
            } else {
                let mut w = wave.clone();
                w.status = WaveStatus::Running as i32;
                let _ = store.update_wave(&w);
                tracing::info!(wave_id = %wave.id, cron = %cron_expr, "cron: activated");
            }
        }
    }
}

fn should_activate_cron(cron_expr: &str, last_run_ended: Option<DateTime<Utc>>) -> bool {
    let schedule = match cron::Schedule::from_str(cron_expr) {
        Ok(s) => s,
        Err(_) => return false,
    };

    let now = Utc::now();
    let grace_period = chrono::Duration::hours(24);
    let check_from = last_run_ended.unwrap_or(now - grace_period);

    // Find scheduled times between check_from and now
    for scheduled in schedule.after(&check_from) {
        if scheduled > now {
            break;
        }
        // Passed a scheduled time without running
        return true;
    }

    false
}
```

## Recovery loop detail

```rust
// loops/recovery.rs

pub fn spawn_recovery_loop(store: SharedStore, cancel: CancellationToken) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(60));
        loop {
            tokio::select! {
                _ = cancel.cancelled() => {
                    tracing::info!("recovery_loop shutting down");
                    break;
                }
                _ = interval.tick() => {
                    recover_stuck_runs(&store);
                }
            }
        }
    })
}

fn recover_stuck_runs(store: &SharedStore) {
    let stuck_threshold = 4 * 60 * 60; // 4 hours in seconds
    let stuck_runs = match store.get_stuck_step_runs(stuck_threshold) {
        Ok(r) => r,
        Err(e) => {
            tracing::error!("failed to query stuck runs: {}", e);
            return;
        }
    };

    for run in stuck_runs {
        tracing::warn!(
            step_run_id = %run.id,
            pid = ?run.pid,
            "step run stuck >4h, terminating"
        );

        // Kill process if PID known
        if let Some(pid) = run.pid {
            let _ = kill_process(pid);
        }

        // Mark step run failed
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        let _ = store.end_step_run(&run.id, StepRunStatus::Failed as i32, now);

        // Update wave failures
        if let Some(wave_id) = &run.wave_id {
            if let Ok(Some(mut wave)) = store.get_wave(wave_id) {
                wave.consecutive_failures += 1;
                if wave.consecutive_failures >= 3 {
                    wave.status = WaveStatus::Error as i32;
                    tracing::error!(wave_id = %wave_id, "wave entered error after 3 failures");
                }
                let _ = store.update_wave(&wave);
            }
        }
    }
}

fn kill_process(pid: u32) -> std::io::Result<()> {
    use std::process::Command;
    Command::new("kill")
        .args(["-TERM", &pid.to_string()])
        .status()?;
    Ok(())
}
```

## Proto additions

Add to `control.proto` Wave message:

```protobuf
message Wave {
  // ... existing fields ...
  uint32 step_index = 20;  // Current position in flow
}
```

## Dependencies

Add to `rust/lfd/Cargo.toml`:

```toml
cron = "0.12"
git2 = "0.18"
tokio-util = { version = "0.7", features = ["rt"] }
chrono = { version = "0.4", features = ["serde"] }
```

## Done when

```bash
# Daemon starts with loops running
cargo run --bin lfd &
# Logs: "starting loop_ticker", "starting watch_poller", "starting cron_poller", "starting recovery_loop"

# Loop wave ticks every 5s
lfd create test --repo . --stimulus loop
lfd run test
# Logs show periodic "tick_loop_waves" activity

# Watch wave activates on git change
lfd create watcher --repo . --stimulus watch --area src/
git commit --allow-empty -m "trigger" && git push
# Within 30s: "watch: activated"

# Cron wave activates on schedule
lfd create scheduler --repo . --stimulus cron --cron "*/5 * * * *"
# At next 5-minute mark: "cron: activated"

# Graceful shutdown drains loops
kill -TERM $(pgrep lfd)
# Logs: "shutdown signal received", "loop_ticker shutting down", ...
```

## Wave alignment

Per the Rust roadmap wave principles:

- **Protocol first:** Loops use store (not new RPC endpoints). Wave/StepRun proto types unchanged except `step_index` addition.
- **Control/execution isolation:** Loops are control plane; tick_flow is execution. A failure in tick_flow marks the wave failed but doesn't crash the loop.
- **SQLite for local:** Loops work with the existing SQLite store. No Postgres dependency in Stage 3.
