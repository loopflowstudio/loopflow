# Rust lfd: Daemon service, session connect, and fork execution

## Purpose
Build the Rust daemon that owns scheduling, run orchestration, and session connect while keeping execution isolated. The daemon is the long-lived control plane with protocol-first APIs and predictable polling loops for external stimuli.

## Current state (2026-01-28)
- Control plane skeleton exists (gRPC endpoints, store, proto types).
- Stimulus loops (loop/watch/cron/recovery) are implemented with store-first state.
- Fork execution paths and worktree handling are in the Rust core, but flow parity is incomplete.
- Session connect is still local-only and not streaming to remote clients.
- `tick_flow` still lacks choose/loop_until_empty; only step and fork are supported.

## Goals
- Make waves execute end-to-end for loop, watch, and cron.
- Unblock interactive steps via session connect.
- Unblock roadmap flows via fork + synthesize.
- Preserve protocol compatibility and UX invariants.

## Architecture
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

## Session connect
Add a session connect path that unblocks interactive steps and establishes the control/execution boundary.

Target behavior:
1. `ConnectWave(wave_id)` attaches to a waiting interactive step.
2. The step runs in a PTY and streams output to the client.
3. Input is accepted via a stream or follow-up RPC.
4. On step exit, `EndStepRun` is called and flow execution resumes.

Key invariant: the daemon remains the source of truth. The connected terminal is a view, not the controller. If the connection drops, the step continues running; reconnection reattaches to the same PTY.

Implementation notes:
- `sessions.rs` owns PTY lifecycle (`portable-pty`).
- `Scheduler` tracks active sessions (wave_id -> session handle).
- `ConnectWave` and `DisconnectWave` are server RPCs.
- `loop_ticker` skips `WaveWaiting` when a session is active.

## Fork execution
Extend `tick_flow` to handle `FlowItem::Fork`:
1. Spawn each branch as a separate worktree.
2. Track branch state in `fork_runs` (parent_wave_id, branch_index, status).
3. Tick each branch independently.
4. When all branches complete, run synthesize (if present).
5. Advance the parent wave past the fork item.

Retry semantics: branches share the wave-level `consecutive_failures` counter; 3 consecutive failures -> `WaveError`.

Choose and loop_until_empty remain out of scope for this wave.

## Scope
In scope:
- Tonic gRPC server + minimal HTTP endpoints
- Loop/watch/cron/recovery background tasks
- Concurrency limiting via global semaphore
- Session connect for interactive steps
- Fork execution + synthesize
- `RunStore` trait + SQLite implementation

Out of scope:
- Cluster/multi-node and auth/tenant isolation
- Postgres backend
- Container isolation
- Full observability stack
- Choose and loop_until_empty flow items

## Risks and bottlenecks
- Watch polling assumes `origin/main`; repos with different default branches won't trigger.
- Cron polling scans step runs each pass; may need indexing/caching at scale.
- Session connect is local-only; remote streaming still TBD.

## Done when

```bash
# Loop wave ticks every 5s
cargo run --bin lfd &
lfd create test --repo . --stimulus loop
lfd run test

# Watch wave activates on git change
lfd create watcher --repo . --stimulus watch --area src/
git commit --allow-empty -m "trigger" && git push

# Cron wave activates on schedule
lfd create scheduler --repo . --stimulus cron --cron "*/5 * * * *"

# Interactive step completes via session connect
lfd create interactive-test --repo . --stimulus manual
lfd run interactive-test
lfd connect interactive-test

# Fork flow executes parallel branches
lfd create fork-test --repo . --flow roadmap-reduce --stimulus manual
lfd run fork-test
lfd status fork-test  # shows WaveIdle after completion

# Graceful shutdown drains loops
kill -TERM $(pgrep lfd)
```
