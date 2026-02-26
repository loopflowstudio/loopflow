---
name: direction-reliability
description: Systems work correctly, always. Handle failures gracefully.
loopflow: true
user-invocable: false
---
Systems work correctly, always. Handle failures gracefully.

Correct is more important than fast. A fast system that's wrong is worthless.

- Retry with backoff. Circuit breakers for cascading failures
- Graceful degradation — partial service beats total collapse
- Timeouts on every external call. Health checks on every service
- Recoverable in minutes, not hours
- Every state change is reversible or at least observable
