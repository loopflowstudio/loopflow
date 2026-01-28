# Daemon Service (Rust lfd)

## Problem

The Python daemon has reliability issues for 24/7 operation: GIL contention under load, unpredictable async behavior, and no clear control/execution boundary. Stage 3 builds the Rust daemon that owns scheduling, run orchestration, and session connect.

Users benefit from: stable long-running operation, predictable scheduling, and the ability to connect to interactive steps from any client (CLI, Concerto, mobile).

Why now: lf-core (Stage 2) provides tick-based execution. The protocol (Stage 1) defines the surface. The daemon is the glue.

## Approach

Build `lfd` as a Tonic gRPC server with minimal HTTP endpoints (health/status/metrics), using the existing control plane protocol. The daemon uses event-driven flow advancement with polling for external state observation.

**Scheduling model:**

| Concern | Mechanism | Interval |
|---------|-----------|----------|
| Flow advancement | Event (step complete, session end) | Immediate |
| Session connect/disconnect | Event (RPC) | Immediate |
| Watch stimulus (git changes) | Poll | 30s |
| Cron stimulus | Poll | 30s |
| PR state (GitHub) | Adaptive poll | 10-300s |
| Stuck run recovery | Safety net poll | 60s |

**Mental model:** Events for internal state transitions. Polling for external state observation. Safety net for resilience.

**Transport:** Tonic for gRPC (primary), tower-http for minimal HTTP endpoints (health/status/metrics).

**Stuck run recovery:** Log long-running steps with elapsed time + PID. If the daemon is at capacity and the oldest step has been running for more than 4 hours, terminate that worker to free a slot and record a failure. Total retries per step/run are capped at 3; exceeding the cap fails the run.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Axum-only (REST) | Simpler setup | Protocol-first principle requires gRPC; Tonic is the standard |
| Actix actors | Natural concurrency model | Heavier abstraction; event + poll model is simpler than actor messaging |
| Pure polling (500ms tick loop) | Dead simple | Wasteful when idle, conflates flow advancement with stimulus detection |
| Pure event-driven | Efficient, responsive | If events are dropped, runs get stuck; need safety net |
| Postgres everywhere | One backend | Local dev needs Docker; SQLite is zero-dependency for single-user |
| Process-per-run isolation | Strong isolation | Overkill for Stage 3; defer to Stage 6 (containers) |

## Key decisions

**Protocol first.** Per wave principle, the daemon implements the control.proto service exactly. No REST-only endpoints that bypass the protocol.

**Events for internal, polling for external.** Flow advancement is event-driven (immediate response when steps complete or sessions end). External state (git, GitHub, cron schedules) is polled at appropriate intervals. A 60s safety net poll catches stuck runs if events are dropped.

**Local-only transport (Stage 3).** No auth in this stage; bind to localhost only. Remote access and API keys are deferred to a later stage.

**Adaptive PR polling.** Match Python's smart intervals: 10s initial, 30s while CI pending, 300s when stable. Don't poll merged/closed PRs.

**SQLite for local, Postgres for containers.** Deployment mode determines backend. Local mode (no containers) uses SQLite for zero dependencies. Containerized mode uses Postgres as the system of record. `RunStore` trait abstracts both from day 1.

**Global concurrency limits.** Single global semaphore: `max(1, num_cpus/2)` concurrent ticks. Prevents thundering herd on restart. RwLock on RunStore prevents double-ticking the same run. Future: per-wave or per-user limits can layer on top without redesign.

**CancellationToken for shutdown.** Graceful shutdown drains in-flight ticks before closing. No orphaned runs.

**Subprocess workers for execution isolation.** Daemon spawns `lfd-worker` process for each run. If worker crashes, daemon marks step failed and continues. Control plane (daemon) stays stable regardless of execution failures. Sets up cleanly for container isolation in Stage 6.

## Scope

In scope:
- Tonic gRPC server implementing control.proto
- Minimal HTTP endpoints (health/status/metrics) via tower-http
- Event-driven flow advancement + polling for external state
- Adaptive PR polling (10-300s based on CI state)
- 60s safety net poll for stuck run recovery
- Concurrency limiting (semaphore)
- Session connect flow for interactive steps
- Graceful shutdown with run reconciliation
- RunStore trait with SQLite implementation
- Structured logging (tracing crate)
- Basic metrics (run count, tick latency, queue depth)
- Event replay mode for parity validation against Python

Out of scope:
- Cluster/multi-node (wave non-goal)
- Enterprise auth beyond API keys (wave non-goal)
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
├── store/
│   ├── mod.rs        # RunStore trait
│   └── sqlite.rs     # SQLite implementation (rusqlite)
├── obs.rs            # Logging, metrics, tracing spans
└── worker/
    └── main.rs       # Worker binary (lfd-worker), spawned per-run
```

- Postgres implementation (`store/postgres.rs`) added in Stage 5.
- Container isolation replaces subprocess spawning in Stage 6.

## Session connect flow

1. `tick_flow` returns `WaitingInteractive` for an interactive step
2. Daemon sets wave status to `Waiting`, broadcasts `wave.waiting` event
3. Client calls `ConnectWave(wave_id)` → returns worktree path, step name, prompt file, step_run_id
4. Client opens terminal session, user works
5. On exit, client calls `EndStepRun(step_run_id, result)`
6. Daemon advances step index, resumes ticking

**Concurrency model:** Optimistic, first-to-close wins.
- Multiple clients can call `ConnectWave` for same wave — all get same info
- Multiple terminals can work in same worktree simultaneously
- `EndStepRun` is atomic — first call wins, subsequent calls rejected
- No session ownership, connection tracking, or heartbeats needed

**Key invariant:** A wave in `Waiting` state does not tick until `EndStepRun` succeeds.

## Parity validation (shadow mode)

Rust daemon runs in **event replay mode** to prove scheduling parity with Python before becoming default.

**Approach:**
1. Python daemon emits event log: triggers received, scheduling decisions, state transitions
2. Rust daemon replays event log, makes decisions, logs what it *would* do
3. Comparison tool diffs Rust decisions vs Python actuals

**Event log contains:**
- Monotonic `sequence_id` per daemon for deterministic ordering
- Trigger events: `git_hook`, `watch_check`, `cron_check`, `pr_poll`
- Decision events: `wave_started`, `wave_queued`, `step_scheduled`
- State transitions: `wave_status_changed`, `step_run_created`, `step_run_ended`

**Parity criteria:**
- Given same trigger sequence, Rust makes same scheduling decisions
- State transitions occur in same order
- Timing jitter within acceptable bounds (not identical, but close)

## Done when

```bash
# Daemon starts and stays up under load
cargo run --bin lfd &
hey -n 10000 -c 50 http://localhost:8080/status  # No crashes

# Scheduling jitter under 100ms
lfd benchmark --runs 100 --measure jitter

# Session connect works end-to-end
lfd connect <wave-id>  # Opens terminal, completes step, flow continues

# Graceful shutdown preserves state
kill -TERM <pid>  # In-flight ticks complete, no orphaned runs
```
