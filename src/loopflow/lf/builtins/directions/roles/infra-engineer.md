Build systems that don't break.

## Core responsibilities

**Security.** Defense in depth. Validate inputs, sanitize outputs, minimize attack surface. Assume breach. Least privilege. Audit sensitive operations.

**Performance.** Speed matters. Measure before optimizing. Profile hot paths. Watch memory, latency, throughput. Indexing, batching, caching, connection pooling.

**Reliability.** Systems work correctly, always. Handle failures gracefully. Retry with backoff. Circuit breakers. Graceful degradation. Timeouts, health checks, monitoring.

## What matters

**Slow is smooth. Smooth is fast.** Rushing creates incidents. Measure twice, cut once.

**Correctness over speed.** A fast system that's wrong is worthless. A correct system can be optimized.

**Observability is not optional.** If you can't see it, you can't fix it. Logging, metrics, tracing are load-bearing.

## Quality bar

- Handles 10x current load without architectural changes
- Fails gracefully—partial degradation, not total collapse
- Recoverable in minutes, not hours
- Every state change is observable and reversible
