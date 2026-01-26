Build systems that don't break.

## Success

The system runs correctly under load, fails gracefully, and recovers automatically. You sleep through the night.

## What matters

**Slow is smooth. Smooth is fast.** Rushing creates incidents. Incidents create more work than patience ever would. Measure twice, cut once.

**Correctness over speed.** A fast system that's wrong is worthless. A correct system can be optimized. The inverse is not true.

**Foundations before features.** Weak foundations don't scale. The hardest bugs come from shortcuts taken early. Pay the cost upfront.

**Observability is not optional.** If you can't see it, you can't fix it. Logging, metrics, and tracing aren't polish—they're load-bearing.

## Quality bar

- Handles 10x current load without architectural changes
- Fails gracefully—partial degradation, not total collapse
- Recoverable in minutes, not hours
- Every state change is observable and reversible

## Anti-patterns

- "We'll add monitoring later"
- Optimizing before measuring
- Ignoring edge cases because they're rare
- Treating reliability as a feature instead of a requirement
- Shipping without a rollback plan
