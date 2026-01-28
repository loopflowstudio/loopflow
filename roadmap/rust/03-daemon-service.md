# Rust Roadmap: Daemon Service (Stage 3)

Implement the Rust daemon that owns scheduling, triggers, and run orchestration.

## Goal
Deliver a production-grade service that runs 24/7, with stable scheduling, safe concurrency, and clear observability.

## Scope
- Socket/HTTP server with protocol handlers
- Scheduling loops for watch/cron/loop
- Concurrency limits and queueing
- Run lifecycle management
- Session connect for interactive steps
- Metrics, logs, and tracing

## Non-goals
- Cluster multi-node scheduling
- Enterprise authn/z (beyond API keys)

## Key components
- `server`: gRPC/HTTP handlers
- `scheduler`: trigger evaluation
- `queue`: concurrency and backpressure
- `runs`: persistence and state transitions
- `sessions`: interactive step connect lifecycle
- `obs`: logs + metrics + tracing

## Session connect
When a flow pauses at an interactive step, users can connect to complete it.

**Connect flow:**
1. Flow hits interactive step, daemon sets `WaveStatus::Waiting`
2. Daemon broadcasts `wave.waiting` event with step details
3. User runs `lfd connect <wave-id>` or clicks Connect in Concerto
4. `/waves/{id}/connect` returns: worktree path, step name, step_run_id, prompt file
5. CLI opens terminal with step prompt, user completes session
6. On exit, CLI calls `StepRunEnd(step_run_id, result)`
7. Daemon advances `FlowRun.step_index`, calls `tick_flow` to continue

**Terminal attachment:**
- CLI: `lfd connect` execs into `lf --step <step> --worktree <path>`
- Concerto: returns prompt file path, opens in embedded terminal or external app

## Reliability requirements
- Explicit timeouts on all external operations
- Graceful shutdown with run reconciliation
- Circuit breaker for repeated failures

## Success criteria
- Stable long-running operation under synthetic load.
- Trigger evaluation jitter below agreed threshold.
- Clear, structured logs for every run.

## Open questions
- Do we keep SQLite for v1 or jump to Postgres now?
- How do we handle per-tenant isolation in early phases?

