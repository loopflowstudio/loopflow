# 03: Iteration Counter Reset

**Finish line:** `max_iterations` counts per-cycle, not per-wave-lifetime. A cron wave that completes one QA cycle and starts another gets a fresh counter.

## Context

The branch construct uses `max_iterations` as a safety valve for QA→fix→deploy loops. Today `wave.iteration` increments monotonically — the counter never resets between cron cycles. This means the safety valve fires based on total lifetime iterations rather than iterations within the current cycle.

Low urgency: `max_iterations` defaults to `None` and won't bite until someone configures it and runs multiple cycles. But it's a correctness issue that should be fixed before QA waves are used in production.

## What to build

- Reset `wave.iteration` when a cron cycle completes (wave state cleared by deploy flow's `update-wave`)
- Or: reset on next cron tick when starting a new cycle
- Either approach works. The key invariant: `max_iterations` bounds iterations within a single cycle, not across the wave's lifetime
