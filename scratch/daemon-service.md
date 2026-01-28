# Rust lfd: Daemon service + trigger loops

## Purpose
Build the Rust daemon that owns scheduling, run orchestration, and session connect while keeping execution isolated. The daemon runs as a long-lived control plane with protocol-first APIs and predictable polling loops for external stimuli.

## Why now
The control plane skeleton (gRPC endpoints, store, proto types) exists, but waves do not execute. Trigger loops are the engine that moves waves forward and must match Python daemon behavior.

## Approach
Build `lfd` as a Tonic gRPC server with minimal HTTP endpoints (health/status/metrics). Use event-driven flow advancement for internal state and polling loops for external state observation.

### Scheduling model

| Concern | Mechanism | Interval |
|---------|-----------|----------|
| Flow advancement | Event (step complete, session end) | Immediate |
| Session connect/disconnect | Event (RPC) | Immediate |
| Watch stimulus (git changes) | Poll | 30s |
| Cron stimulus | Poll | 30s |
| PR state (GitHub) | Adaptive poll | 10-300s |
| Stuck run recovery | Safety net poll | 60s |

Mental model: events for internal state transitions, polling for external state observation, and a safety net for resilience.

## Trigger loops
Four background tokio tasks, started via `Scheduler::start_loops()` and coordinated via `CancellationToken`.

| Loop | Interval | Responsibility |
|------|----------|----------------|
| `loop_ticker` | 5s | Tick waves with `StimulusKind::Loop` in `Running` |
| `watch_poller` | 30s | Check git changes for `StimulusKind::Watch` |
| `cron_poller` | 30s | Evaluate cron schedules for `StimulusKind::Cron` |
| `recovery_loop` | 60s | Find step runs stuck >4h, terminate + retry/fail |

Key invariant: a wave in `Waiting` does not tick until `EndStepRun` succeeds.

### Loop mechanics
1. Query store for relevant waves
2. Evaluate trigger condition
3. Acquire slot via semaphore (loop_ticker) or queue activation (watch/cron)
4. Call `lf-core::tick_flow` or mark wave for activation
5. Update store with result
6. Respect cancellation token on shutdown

### Watch polling
- Use `git2` to fetch `origin/main`, compare SHA, and diff for area path matches.
- If change is outside area, update SHA without activation.

### Cron polling
- Use `cron` crate to evaluate schedules.
- Use last ended step run (or a 24h grace period) to detect missed schedules.

### Recovery loop
- Find step runs older than 4 hours.
- If PID is known, terminate the worker (graceful signal first).
- Mark step run failed and update wave failure counters.

## Key decisions
- Protocol first: implement `control.proto` exactly; no REST-only bypasses.
- Store-first state: loops read/write SQLite state, no in-memory wave state.
- Slot acquisition only in `loop_ticker` to honor global concurrency limits.
- Retry cap at wave level using `consecutive_failures`; 3 consecutive failures -> `Error`.
- SQLite for local mode, Postgres later (Stage 5).

## Scope
In scope:
- Tonic gRPC server + minimal HTTP endpoints
- Loop/watch/cron/recovery background tasks
- Concurrency limiting via global semaphore
- Session connect flow for interactive steps
- Graceful shutdown with run reconciliation
- `RunStore` trait + SQLite implementation
- Event replay mode for parity validation

Out of scope:
- Cluster/multi-node, enterprise auth (later stages)
- Postgres backend (Stage 5)
- Container isolation (Stage 6)
- Full observability stack (dashboards, alerting)

## Components

```
rust/lfd/
├── main.rs           # Daemon entry point, signal handling
├── server.rs         # Tonic service impl
├── scheduler.rs      # Event handlers, polling loops, semaphore
├── sessions.rs       # Interactive step connect lifecycle
├── loops/
│   ├── mod.rs
│   ├── loop_ticker.rs
│   ├── watch.rs
│   ├── cron.rs
│   └── recovery.rs
├── store/
│   ├── mod.rs        # RunStore trait
│   └── sqlite.rs     # SQLite implementation (rusqlite)
├── obs.rs            # Logging, metrics, tracing spans
└── worker/
    └── main.rs       # Worker binary (lfd-worker), spawned per-run
```

## Status and risks (2026-01-28)
- `tick_flow` only supports linear steps; fork/choose/loop items still fail.
- Watch polling only inspects `origin/main`; repos with different default branches won't trigger.
- Cron polling reads all step runs each pass; may be slow at scale.
- Session connect/event streaming not yet implemented; interactive flows are not end-to-end.

## Done when

```bash
cargo run --bin lfd &

# Loop wave ticks every 5s
lfd create test --repo . --stimulus loop
lfd run test

# Watch wave activates on git change
lfd create watcher --repo . --stimulus watch --area src/
git commit --allow-empty -m "trigger" && git push

# Cron wave activates on schedule
lfd create scheduler --repo . --stimulus cron --cron "*/5 * * * *"

# Graceful shutdown drains loops
kill -TERM $(pgrep lfd)
```
