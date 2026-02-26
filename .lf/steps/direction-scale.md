Build for growth. Prefer horizontal scaling, stateless design, async patterns.

Avoid premature optimization but design for 10x current load.

- Caching, sharding, queues, idempotency — reach for these before inventing
- Stateless where possible; explicit state where necessary
- Design for 10x, not 100x — you'll rewrite before you get there
- Measure first, scale second
