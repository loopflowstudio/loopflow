# Trigger Evaluation Loops

Implement the scheduling loops that drive wave execution.

## Problem

The Rust daemon has gRPC endpoints and a store but no active scheduling. Waves can be created and their status changed, but nothing actually runs them. The daemon is passive.

## Scope

From the daemon-service design doc:

| Concern | Mechanism | Interval |
|---------|-----------|----------|
| Flow advancement | Event (step complete, session end) | Immediate |
| Session connect/disconnect | Event (RPC) | Immediate |
| Watch stimulus (git changes) | Poll | 30s |
| Cron stimulus | Poll | 30s |
| PR state (GitHub) | Adaptive poll | 10-300s |
| Stuck run recovery | Safety net poll | 60s |

**This work focuses on the polling loops.** Event-driven advancement already has RPC endpoints (RunWave, StopWave, EndStepRun). The polling loops need to:

1. Periodically check waves with `StimulusKind::Loop` and tick them
2. Periodically check waves with `StimulusKind::Watch` for git changes
3. Periodically check waves with `StimulusKind::Cron` for schedule matches
4. Periodically check for stuck runs and recover

## Key components

```rust
// scheduler.rs additions

impl Scheduler {
    /// Start background polling tasks. Returns JoinHandles for graceful shutdown.
    pub fn start_loops(
        self: Arc<Self>,
        store: SharedStore,
        cancel: CancellationToken,
    ) -> Vec<JoinHandle<()>>;
}
```

Each loop runs as a separate tokio task:

- **loop_ticker**: Every 5s, find waves with `StimulusKind::Loop` in `Running` status, attempt to tick
- **watch_poller**: Every 30s, find waves with `StimulusKind::Watch`, check git for changes, mark pending
- **cron_poller**: Every 30s, find waves with `StimulusKind::Cron`, check if due, mark pending
- **recovery_loop**: Every 60s, find step runs stuck for >4h, terminate and retry or fail

## Flow tick integration

When a loop determines a wave should tick, it needs to call into lf-core:

```rust
// In scheduler tick logic
let result = lf_core::tick_flow(&flow_state, &worktree);
match result {
    TickResult::StepComplete { .. } => { /* advance, save state */ }
    TickResult::WaitingInteractive { .. } => { /* set wave to Waiting */ }
    TickResult::FlowComplete => { /* mark done, create PR if configured */ }
    TickResult::Error { .. } => { /* increment failures, maybe circuit break */ }
}
```

This requires:
- `lfd` depends on `lf-core`
- Flow state persisted in store (current step index, worktree path)
- lf-core exposes a sync or async tick interface

## Graceful shutdown

Use `tokio_util::sync::CancellationToken`:

```rust
// main.rs
let cancel = CancellationToken::new();
let scheduler_handles = scheduler.start_loops(store.clone(), cancel.clone());

tokio::select! {
    _ = signal::ctrl_c() => {
        cancel.cancel();
        for handle in scheduler_handles {
            handle.await.ok();
        }
    }
    // ... server tasks
}
```

## Success criteria

```bash
# Create a wave with loop stimulus, see it tick
lfd create test-wave --repo . --area src/ --stimulus loop
# Watch logs show periodic tick attempts

# Graceful shutdown drains in-flight work
kill -TERM $(pgrep lfd)
# Logs show "shutdown signal received, draining..."
```

## Open questions

- Should watch polling use git fetch or just stat the local ref? (Leaning: git fetch for accuracy)
- How much lf-core API surface do we expose vs keep internal? (Leaning: minimal public tick interface)
- Do we persist flow state in existing Wave proto or add FlowRun table? (Leaning: separate FlowRun)
