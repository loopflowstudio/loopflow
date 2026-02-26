---
name: direction-performance
description: Speed matters. Measure before optimizing. Profile hot paths.
loopflow: true
user-invocable: false
---
Speed matters. Measure before optimizing. Profile hot paths.

Watch memory, latency, throughput. Know which operations are hot and which are cold.

- Measure first, optimize second — intuition about bottlenecks is usually wrong
- Indexing, batching, caching, connection pooling — reach for these before inventing
- Set latency budgets for user-facing operations
- Allocation patterns matter more than micro-optimizations
