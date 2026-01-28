# Rust Roadmap: Daemon Service (Stage 3)

Implement the Rust daemon that owns scheduling, triggers, and run orchestration.

## Goal
Deliver a production-grade service that runs 24/7, with stable scheduling, safe concurrency, and clear observability.

## Scope
- Socket/HTTP server with protocol handlers
- Scheduling loops for watch/cron/loop
- Concurrency limits and queueing
- Run lifecycle management
- Metrics, logs, and tracing

## Non-goals
- Cluster multi-node scheduling
- Enterprise authn/z (beyond API keys)

## Key components
- `server`: gRPC/HTTP handlers
- `scheduler`: trigger evaluation
- `queue`: concurrency and backpressure
- `runs`: persistence and state transitions
- `obs`: logs + metrics + tracing

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

